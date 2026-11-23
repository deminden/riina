//! Confine the settings bridge to one installed package and one browser origin.
use crate::platform::plugin_resources::{
    plugin_file_url, resource_response, trusted_origin, webview_navigation_url,
};
use crate::plugin_runtime::PluginPreferences;
use riina_core::plugin::Manifest;
use serde::Deserialize;
use serde_json::Value;
use std::{
    borrow::Cow,
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};
use url::Url;
use uuid::Uuid;
use wry::{
    NewWindowResponse, WebView, WebViewBuilder,
    http::{Request, Response, StatusCode},
};

const SCHEME: &str = "riina-plugin";
const MAX_REQUEST_BYTES: usize = 1024 * 1024 + 512;
const BRIDGE: &str = include_str!("bridge.js");

pub(super) struct Document {
    root: PathBuf,
    authority: String,
    url: String,
    name: String,
    preferences: PluginPreferences,
    reads: RefCell<Vec<(u32, String)>>,
}

impl Document {
    pub(super) fn new(package: &Path, manifest: &Manifest, id: Uuid) -> Result<Self, String> {
        let root = package.canonicalize().map_err(|error| error.to_string())?;
        let path = manifest
            .preferences_page
            .as_deref()
            .ok_or("This plugin has no settings page")?;
        let file = root
            .join(path)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err("The settings page must be a file inside the installed plugin".into());
        }
        let authority = format!("settings-{}.localhost", id.simple());
        let mut url = Url::parse(&plugin_file_url(path)?).map_err(|error| error.to_string())?;
        url.set_host(Some(&authority))
            .map_err(|error| error.to_string())?;
        // A linked package serves resources from its target but stores edits
        // beside its installed entry, just like the plugin runtime does.
        let preferences = PluginPreferences::for_package(package, manifest)?;
        Ok(Self {
            root,
            authority,
            url: webview_navigation_url(url.into()),
            name: manifest.name.clone(),
            preferences,
            reads: RefCell::default(),
        })
    }

    pub(super) fn builder(self: &Rc<Self>) -> WebViewBuilder<'static> {
        let root = self.root.clone();
        let authority = self.authority.clone();
        let url = self.url.clone();
        let target = Rc::downgrade(self);
        WebViewBuilder::new()
            .with_url(&self.url)
            .with_initialization_script_for_main_only(BRIDGE, true)
            .with_custom_protocol(SCHEME.into(), move |_, request| {
                if trusted_origin(&request.uri().to_string(), &authority) {
                    resource_response(&root, request)
                } else {
                    let mut response = Response::new(Cow::Borrowed(&[][..]));
                    *response.status_mut() = StatusCode::NOT_FOUND;
                    response
                }
            })
            .with_navigation_handler(move |candidate| {
                candidate.split('#').next() == Some(url.as_str())
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_ipc_handler(move |request: Request<String>| {
                if let Some(document) = target.upgrade() {
                    document.receive(request);
                }
            })
    }

    fn receive(&self, request: Request<String>) {
        if request.body().len() > MAX_REQUEST_BYTES
            || !trusted_origin(&request.uri().to_string(), &self.authority)
        {
            return;
        }
        let Ok(message) = serde_json::from_str(request.body()) else {
            return;
        };
        match message {
            Message::Get { id, key } => {
                let mut reads = self.reads.borrow_mut();
                if reads.len() < 128 && key.len() <= 4096 {
                    reads.push((id, key));
                }
            }
            Message::Set { key, value } => {
                if let Err(error) = self.preferences.set(key, value) {
                    crate::diagnostics::warn!("{} plugin settings: {error}", self.name);
                }
            }
            Message::Log { message } => {
                if message.len() <= 4096 {
                    crate::diagnostics::log_args(
                        crate::diagnostics::Level::Info,
                        "plugin settings",
                        format_args!("{}: {message}", self.name),
                    );
                }
            }
        }
    }

    pub(super) fn flush(&self, browser: &WebView) -> Result<(), String> {
        let reads = self.reads.take();
        for (id, key) in reads {
            let value = self.preferences.get(&key)?;
            // JSON.parse preserves JSON object keys such as __proto__ exactly.
            let value =
                serde_json::to_string(&value.to_string()).map_err(|error| error.to_string())?;
            browser
                .evaluate_script(&format!(
                    "window.iina?._receivePreference({id}, JSON.parse({value}));"
                ))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(super) fn sync(&self) -> Result<(), String> {
        self.preferences.sync()
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Message {
    Get { id: u32, key: String },
    Set { key: String, value: Value },
    Log { message: String },
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn settings_of_a_linked_package_share_the_installed_store() {
        let scratch = riina_test_support::TempDir::new("linked-plugin-settings").unwrap();
        let plugins = scratch.path().join("Plugins");
        let external = scratch.path().join("source");
        fs::create_dir(&plugins).unwrap();
        fs::create_dir(&external).unwrap();
        fs::write(external.join("settings.html"), "<!doctype html>").unwrap();
        let installed = plugins.join("org.example.settings.iinaplugin");
        symlink(&external, &installed).unwrap();
        let manifest = Manifest::parse(
            r#"{
            "name":"Test", "identifier":"org.example.settings", "version":"1",
            "author":{"name":"Test"}, "entry":"main.js", "preferencesPage":"settings.html"
        }"#,
        )
        .unwrap();
        let runtime = PluginPreferences::for_package(&installed, &manifest).unwrap();
        let document = Document::new(&installed, &manifest, Uuid::nil()).unwrap();
        let value = Value::Bool(true);
        document
            .preferences
            .set("enabled".into(), value.clone())
            .unwrap();
        assert_eq!(runtime.get("enabled").unwrap(), value);
        document.sync().unwrap();
        assert!(
            plugins
                .join(".preferences/org.example.settings.json")
                .is_file()
        );
        assert!(!scratch.path().join(".preferences").exists());
    }
}
