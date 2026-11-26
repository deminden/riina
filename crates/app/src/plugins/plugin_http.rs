//! One bounded redirect and credential policy for every plugin HTTP transport.
use crate::http::{RequestPolicy, agent_config};
use riina_core::plugin::{
    NetworkGrant, network_address_is_restricted, network_host_is_restricted, network_url_grant,
};
use std::time::{Duration, Instant};
use ureq::unversioned::{
    resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver},
    transport::{DefaultConnector, NextTimeout},
};
mod xmlrpc;

pub(crate) use xmlrpc::{XmlrpcReply, perform_xmlrpc_call};

const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_NETWORK_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_DOWNLOAD_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn perform_http_request(
    method: &str,
    raw_url: &str,
    options: &serde_json::Value,
    domains: &[String],
) -> Result<serde_json::Value, String> {
    let mut response = request(method, raw_url, options, domains)?;
    let status = response.status();
    let text = response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Could not read HTTP response: {e}"))?;
    Ok(serde_json::json!({
        "statusCode": status.as_u16(),
        "reason": status.canonical_reason().unwrap_or(""),
        "data": serde_json::from_str::<serde_json::Value>(&text).unwrap_or(serde_json::Value::Null),
        "text": text,
    }))
}

pub(crate) fn perform_http_download(
    raw_url: &str,
    options: &serde_json::Value,
    domains: &[String],
) -> Result<Vec<u8>, String> {
    let method = options
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("GET");
    let mut response = request(method, raw_url, options, domains)?;
    if !response.status().is_success() {
        return Err(format!(
            "HTTP download failed with status {}",
            response.status()
        ));
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit((MAX_PLUGIN_DOWNLOAD_BYTES + 1) as u64)
        .read_to_vec()
        .map_err(|e| format!("Could not read HTTP download: {e}"))?;
    if bytes.len() > MAX_PLUGIN_DOWNLOAD_BYTES {
        return Err("Download exceeds the 64 MiB limit".into());
    }
    Ok(bytes)
}

fn request(
    method: &str,
    raw_url: &str,
    options: &serde_json::Value,
    domains: &[String],
) -> Result<ureq::http::Response<ureq::Body>, String> {
    let mut url = url::Url::parse(raw_url).map_err(|e| e.to_string())?;
    if let Some(params) = options.get("params") {
        let params = params.as_object().ok_or("HTTP params must be an object")?;
        let mut query = url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, &option_string(value));
        }
    }
    let method = ureq::http::Method::from_bytes(method.as_bytes())
        .map_err(|e| format!("Invalid HTTP method: {e}"))?;
    let mut body = Vec::new();
    let data = options.get("data").filter(|v| !v.is_null());
    if let Some(data) = data {
        let fields = data.as_object().ok_or("HTTP data must be an object")?;
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in fields {
            form.append_pair(key, &option_string(value));
        }
        body = form.finish().into_bytes();
    }
    if body.len() > MAX_NETWORK_REQUEST_BYTES {
        return Err("HTTP request body exceeds the 1 MiB limit".into());
    }
    let mut headers = ureq::http::HeaderMap::new();
    if let Some(values) = options.get("headers") {
        for (name, value) in values.as_object().ok_or("HTTP headers must be an object")? {
            let name =
                ureq::http::HeaderName::from_bytes(name.as_bytes()).map_err(|e| e.to_string())?;
            let value = ureq::http::HeaderValue::from_str(&option_string(value))
                .map_err(|e| e.to_string())?;
            headers.insert(name, value);
        }
    }
    if data.is_some() && !headers.contains_key(ureq::http::header::CONTENT_TYPE) {
        headers.insert(
            ureq::http::header::CONTENT_TYPE,
            ureq::http::HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
    }
    fetch(url, method, headers, body, domains)
}

fn redirect_policy(
    current: &url::Url,
    next: &url::Url,
    headers: &mut ureq::http::HeaderMap,
) -> Result<(), String> {
    if current.origin() != next.origin() {
        for name in [
            "authorization",
            "cookie",
            "proxy-authorization",
            "origin",
            "referer",
        ] {
            headers.remove(name);
        }
    }
    if current.scheme() == "https" && next.scheme() == "http" {
        return Err("HTTPS redirects to insecure HTTP are not allowed".into());
    }
    if is_public_host(current) && !is_public_host(next) {
        return Err("HTTP redirect crossed into a restricted network".into());
    }
    Ok(())
}

fn is_public_host(url: &url::Url) -> bool {
    url.host_str()
        .is_some_and(|host| !network_host_is_restricted(host))
}

#[derive(Debug)]
struct PluginResolver {
    public_only: bool,
}

impl Resolver for PluginResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let addresses = DefaultResolver::default().resolve(uri, config, timeout)?;
        // Proxy addresses are not the destination. The proxy resolves the
        // destination itself, so the address check applies to direct requests.
        if !self.public_only || config.proxy().is_some() {
            return Ok(addresses);
        }
        self.filter_addresses(addresses)
    }
}

impl PluginResolver {
    fn filter_addresses(
        &self,
        addresses: ResolvedSocketAddrs,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let mut allowed = self.empty();
        for &address in &addresses {
            if !network_address_is_restricted(address.ip()) {
                allowed.push(address);
            }
        }
        if allowed.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(allowed)
    }
}

fn fetch(
    mut url: url::Url,
    mut method: ureq::http::Method,
    mut headers: ureq::http::HeaderMap,
    mut body: Vec<u8>,
    domains: &[String],
) -> Result<ureq::http::Response<ureq::Body>, String> {
    let started = Instant::now();
    // Host and message framing belong to the current target and body, not an
    // earlier URL or plugin-supplied transport metadata.
    headers.remove(ureq::http::header::HOST);
    headers.remove(ureq::http::header::CONTENT_LENGTH);
    let mut redirected_from_public_host = false;
    for hop in 0..=10 {
        let Some(grant) = network_url_grant(&url, domains) else {
            return Err("HTTP target is not in the plugin's allowed domains".into());
        };
        let remaining = NETWORK_TIMEOUT.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err("HTTP request exceeded the 30 second timeout".into());
        }
        let agent = ureq::Agent::with_parts(
            agent_config(remaining, RequestPolicy::ManualRedirects),
            DefaultConnector::new(),
            PluginResolver {
                public_only: grant == NetworkGrant::Wildcard || redirected_from_public_host,
            },
        );
        let mut request = ureq::http::Request::builder()
            .method(method.clone())
            .uri(url.as_str())
            .body(body.clone())
            .map_err(|e| e.to_string())?;
        *request.headers_mut() = headers.clone();
        let response = agent
            .run(request)
            .map_err(|e| format!("HTTP request failed: {e}"))?;
        let status = response.status().as_u16();
        if !matches!(status, 301 | 302 | 303 | 307 | 308) {
            return Ok(response);
        }
        let Some(location) = response.headers().get(ureq::http::header::LOCATION) else {
            return Ok(response);
        };
        if hop == 10 {
            break;
        }
        let next = url
            .join(location.to_str().map_err(|e| e.to_string())?)
            .map_err(|e| format!("Invalid HTTP redirect URL: {e}"))?;
        if network_url_grant(&next, domains).is_none() {
            return Err("HTTP redirect target is not in the plugin's allowed domains".into());
        }
        redirect_policy(&url, &next, &mut headers)?;
        redirected_from_public_host = is_public_host(&url);
        if (status == 303 && method != ureq::http::Method::HEAD)
            || (matches!(status, 301 | 302) && method == ureq::http::Method::POST)
        {
            method = ureq::http::Method::GET;
            body.clear();
            for name in [
                "content-type",
                "content-encoding",
                "content-language",
                "content-location",
                "content-length",
                "digest",
            ] {
                headers.remove(name);
            }
        }
        url = next;
    }
    Err("HTTP redirect limit was exceeded".into())
}

fn option_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(v) => v.clone(),
        v => v.to_string(),
    }
}

#[cfg(test)]
#[path = "plugin_http_tests.rs"]
mod tests;
