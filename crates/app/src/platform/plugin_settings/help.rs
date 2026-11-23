//! Render plugin documentation in a browser without a plugin runtime bridge.
use super::super::plugin_resources::{
    plugin_file_url, resource_response, trusted_origin, webview_navigation_url,
};
use riina_core::plugin::{HelpPage, plugin_web_link};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};
use url::Url;
use uuid::Uuid;
use wry::{
    NewWindowResponse, WebViewBuilder,
    http::{Response, StatusCode},
};

pub(super) struct Help {
    url: String,
    package: Option<(PathBuf, String)>,
}

impl Help {
    pub(super) fn new(root: &Path, page: &HelpPage, id: Uuid) -> Result<Self, String> {
        match page {
            HelpPage::Remote(url) => Ok(Self {
                url: url.to_string(),
                package: None,
            }),
            HelpPage::Package(path) => {
                let root = root.canonicalize().map_err(|error| error.to_string())?;
                let file = root
                    .join(path)
                    .canonicalize()
                    .map_err(|error| error.to_string())?;
                if !file.starts_with(&root) || !file.is_file() {
                    return Err("The help page must be a file inside the installed plugin".into());
                }
                let authority = format!("help-{}.localhost", id.simple());
                let mut url =
                    Url::parse(&plugin_file_url(path)?).map_err(|error| error.to_string())?;
                url.set_host(Some(&authority))
                    .map_err(|error| error.to_string())?;
                Ok(Self {
                    url: webview_navigation_url(url.into()),
                    package: Some((root, authority)),
                })
            }
        }
    }

    pub(super) fn builder(&self) -> WebViewBuilder<'static> {
        let builder = WebViewBuilder::new()
            .with_incognito(true)
            .with_url(&self.url)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny);
        let Some((root, authority)) = self.package.clone() else {
            return builder
                .with_navigation_handler(|candidate| plugin_web_link(&candidate).is_some());
        };
        let resource_origin = authority.clone();
        builder
            .with_custom_protocol("riina-plugin".into(), move |_, request| {
                if trusted_origin(&request.uri().to_string(), &resource_origin) {
                    resource_response(&root, request)
                } else {
                    let mut response = Response::new(Cow::Borrowed(&[][..]));
                    *response.status_mut() = StatusCode::NOT_FOUND;
                    response
                }
            })
            .with_navigation_handler(move |candidate| {
                trusted_origin(&candidate, &authority) || plugin_web_link(&candidate).is_some()
            })
    }
}
