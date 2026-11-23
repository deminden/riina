//! Confined custom-protocol documents and document-scoped WebView messages.
use super::plugin_resources::{plugin_file_url, resource_response, webview_navigation_url};
use riina_core::plugin_window::Document;
use std::{borrow::Cow, cell::Cell, path::PathBuf, rc::Rc, sync::Arc};
use url::Url;
use wry::{
    NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder,
    http::{Request, Response, StatusCode},
};

const SCHEME: &str = "riina-plugin";
const SHELL: &str = "/.riina-plugin-page";
const MAX_IPC_BYTES: usize = 1024 * 1024 + 512;
const BRIDGE: &str = include_str!("plugin_page_bridge.js");
const STANDALONE: &[u8] = br#"<!doctype html><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
  :root { color-scheme: light dark; font: 13px system-ui, sans-serif; }
  body { margin: 12px; }
</style>
<style id="plugin-style" data-riina-style></style><main id="plugin-content" data-riina-content></main>"#;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
const OVERLAY: &[u8] = br#"<!doctype html><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
  html,body { margin: 0; min-height: 100%; }
  body { font: 13px system-ui, sans-serif; color: white;
         text-shadow: 0 1px black, 0 -1px black, 1px 0 black, -1px 0 black; }
</style>
<style id="style" data-riina-style></style><div id="content" data-riina-content></div>"#;

struct Observation {
    expected: Cell<u64>,
    ready: Cell<bool>,
    page: Cell<u64>,
    awaiting_tagged_page: Cell<bool>,
}

impl Observation {
    fn observe(&self, event: PageLoadEvent, tag: Option<u64>) -> bool {
        let checked = tag == Some(self.expected.get());
        if !checked && (tag.is_some() || self.awaiting_tagged_page.get()) {
            return false;
        }
        let ready = matches!(event, PageLoadEvent::Finished);
        self.ready.set(ready);
        if ready {
            self.awaiting_tagged_page.set(false);
            self.page.set(self.page.get().wrapping_add(1));
        }
        ready
    }
}

pub(super) struct Page {
    state: Rc<Observation>,
    authority: Arc<str>,
    simple: Option<&'static [u8]>,
}

impl Page {
    pub(super) fn standalone(document: u64, origin: uuid::Uuid) -> Self {
        Self::new(document, origin, Some(STANDALONE))
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub(super) fn overlay(document: u64) -> Self {
        Self::new(document, uuid::Uuid::new_v4(), Some(OVERLAY))
    }

    pub(super) fn sidebar(document: u64) -> Self {
        Self::new(document, uuid::Uuid::new_v4(), None)
    }

    fn new(document: u64, origin: uuid::Uuid, simple: Option<&'static [u8]>) -> Self {
        Self {
            state: Rc::new(Observation {
                expected: Cell::new(document),
                ready: Cell::new(false),
                page: Cell::new(0),
                awaiting_tagged_page: Cell::new(true),
            }),
            authority: format!("player-plugin-{}.localhost", origin.simple()).into(),
            simple,
        }
    }

    pub(super) fn navigating_to(&self, document: u64) {
        self.state.expected.set(document);
        self.state.ready.set(false);
        self.state.awaiting_tagged_page.set(true);
    }

    pub(super) fn ready_page(&self) -> Option<u64> {
        self.state.ready.get().then(|| self.state.page.get())
    }

    pub(super) fn builder<'a>(
        &self,
        root: Option<PathBuf>,
        initial_url: &str,
        receive: impl Fn(u64, String, serde_json::Value) + 'static,
        loaded: impl Fn() + 'static,
    ) -> WebViewBuilder<'a> {
        let state = self.state.clone();
        let active_document = self.state.clone();
        let simple = self.simple;
        let protocol_origin = self.authority.clone();
        let navigation_origin = self.authority.clone();
        let page_origin = self.authority.clone();
        let message_origin = self.authority.clone();
        WebViewBuilder::new()
            .with_url(initial_url)
            .with_initialization_script(BRIDGE)
            .with_custom_protocol(SCHEME.into(), move |_, request| {
                if !trusted_origin(&request.uri().to_string(), &protocol_origin) {
                    return unavailable();
                }
                match shell_request(&request) {
                    true => simple.map_or_else(unavailable, simple_response),
                    false => root
                        .as_ref()
                        .map_or_else(unavailable, |root| resource_response(root, request)),
                }
            })
            .with_navigation_handler(move |url| trusted_origin(&url, &navigation_origin))
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_on_page_load_handler(move |event, url| {
                if trusted_origin(&url, &page_origin) && state.observe(event, document_id(&url)) {
                    loaded();
                }
            })
            .with_ipc_handler(move |request: Request<String>| {
                if request.body().len() > MAX_IPC_BYTES
                    || !trusted_origin(&request.uri().to_string(), &message_origin)
                {
                    return;
                }
                let Ok(message) = serde_json::from_str::<Incoming>(request.body()) else {
                    return;
                };
                if let Ok(document) = message.document.parse()
                    && active_document.expected.get() == document
                {
                    receive(document, message.name, message.data);
                }
            })
    }

    pub(super) fn url(&self, document_id: u64, document: &Document) -> Result<String, String> {
        let (base, shell) = match document {
            Document::Package(file) => (plugin_file_url(file)?, false),
            _ if self.simple.is_some() => (format!("{SCHEME}://localhost{SHELL}"), true),
            _ => return Err("Plugin sidebar requires a package document".into()),
        };
        let mut url = Url::parse(&base).map_err(|error| error.to_string())?;
        // Keep storage and script origins distinct across realms, including
        // WebView2 environments with a shared underlying browser profile.
        url.set_host(Some(&self.authority))
            .map_err(|error| error.to_string())?;
        let mut query = url.query_pairs_mut();
        query.append_pair("riina_document", &document_id.to_string());
        if shell {
            query.append_pair("riina_shell", "1");
        }
        drop(query);
        Ok(webview_navigation_url(url.into()))
    }
}

#[derive(serde::Deserialize)]
struct Incoming {
    document: String,
    name: String,
    #[serde(default)]
    data: serde_json::Value,
}

fn trusted_origin(url: &str, authority: &str) -> bool {
    url.strip_prefix("riina-plugin://")
        .or_else(|| url.strip_prefix("http://riina-plugin."))
        .and_then(|url| url.strip_prefix(authority))
        .is_some_and(|path| path.starts_with('/'))
}

fn document_id(url: &str) -> Option<u64> {
    let url = Url::parse(url).ok()?;
    let (_, tag) = url.query_pairs().find(|(key, _)| key == "riina_document")?;
    tag.parse().ok()
}

fn shell_request(request: &Request<Vec<u8>>) -> bool {
    request.uri().path() == SHELL
        && request
            .uri()
            .query()
            .is_some_and(|query| query.split('&').any(|entry| entry == "riina_shell=1"))
}

fn simple_response(body: &'static [u8]) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Content-Security-Policy",
            "default-src 'self' data: blob:; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'")
        .body(Cow::Borrowed(body))
        .unwrap_or_else(|_| unavailable())
}

fn unavailable() -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(Cow::Borrowed(&[][..]));
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
}

pub(super) fn update_simple_document(
    browser: &WebView,
    document: u64,
    previous: Option<&Document>,
    next: &Document,
) -> Result<(), String> {
    let Document::Simple { style, content } = next else {
        return Ok(());
    };
    let (last_style, last_content) = match previous {
        Some(Document::Simple { style, content }) => (style.as_deref(), content.as_deref()),
        _ => (None, None),
    };
    let mut script = String::new();
    if let Some(style) = style.as_deref().filter(|style| Some(*style) != last_style) {
        script.push_str(&format!("const style = document.querySelector('[data-riina-style]'); if (style) style.textContent = {};", json(style)?));
    }
    if let Some(content) = content
        .as_deref()
        .filter(|content| Some(*content) != last_content)
    {
        script.push_str(&format!("const content = document.querySelector('[data-riina-content]'); if (content) content.innerHTML = {};", json(content)?));
    }
    if script.is_empty() {
        return Ok(());
    }
    evaluate(browser, document, &script)
}

pub(super) fn deliver(
    browser: &WebView,
    document: u64,
    name: &str,
    data: &serde_json::Value,
) -> Result<(), String> {
    evaluate(
        browser,
        document,
        &format!(
            "window.iina._emit({}, {});",
            json(name)?,
            json(&json(data)?)?
        ),
    )
}

fn json(value: &(impl serde::Serialize + ?Sized)) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

pub(super) fn evaluate(browser: &WebView, document: u64, script: &str) -> Result<(), String> {
    browser
        .evaluate_script(&format!(
            "if (window.iina?._matchesDocument(\"{document}\")) {{ {script} }}"
        ))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_pages_use_distinct_confined_origins() {
        let first = Page::standalone(7, uuid::Uuid::from_u128(1));
        let other = Page::standalone(7, uuid::Uuid::from_u128(2));
        let document = Document::Package("pages/test café.html".into());
        let url = first.url(7, &document).unwrap();
        assert!(url.contains("/pages/test%20caf%C3%A9.html?"));
        #[cfg(target_os = "windows")]
        assert!(url.starts_with("http://riina-plugin."));
        assert_eq!(document_id(&url), Some(7));
        assert!(trusted_origin(&url, &first.authority));
        assert!(!trusted_origin(&url, &other.authority));
        let native = url.replace("riina-plugin://", "http://riina-plugin.");
        assert!(trusted_origin(&native, &first.authority));
        let misleading = url.replace(".localhost/", ".localhost.example/");
        assert!(!trusted_origin(&misleading, &first.authority));
    }

    #[test]
    fn stale_page_loads_do_not_release_messages_to_the_next_document() {
        let page = Page::standalone(7, uuid::Uuid::from_u128(1));
        let state = &page.state;
        assert!(state.observe(PageLoadEvent::Finished, Some(7)));
        page.navigating_to(8);
        assert!(!state.observe(PageLoadEvent::Finished, Some(7)));
        assert!(!state.observe(PageLoadEvent::Finished, None));
        assert_eq!(page.ready_page(), None);
        assert!(state.observe(PageLoadEvent::Finished, Some(8)));
        assert_eq!(page.ready_page(), Some(2));
    }
}
