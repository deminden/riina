//! One bounded redirect and credential policy for every plugin HTTP transport.
use riina_core::plugin::network_url_allowed;
use std::time::{Duration, Instant};
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
    Ok(
        serde_json::json!({"statusCode":status.as_u16(), "reason":status.canonical_reason().unwrap_or(""), "data":serde_json::from_str::<serde_json::Value>(&text).unwrap_or(serde_json::Value::Null), "text":text}),
    )
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
    Ok(())
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
    for hop in 0..=10 {
        if !network_url_allowed(url.as_str(), domains) {
            return Err("HTTP target is not in the plugin's allowed domains".into());
        }
        let remaining = NETWORK_TIMEOUT.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err("HTTP request exceeded the 30 second timeout".into());
        }
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(remaining))
                .max_redirects(0)
                .http_status_as_error(false)
                .build(),
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
            return Err("HTTP redirect limit was exceeded".into());
        }
        let next = url
            .join(location.to_str().map_err(|e| e.to_string())?)
            .map_err(|e| format!("Invalid HTTP redirect URL: {e}"))?;
        if !network_url_allowed(next.as_str(), domains) {
            return Err("HTTP redirect target is not in the plugin's allowed domains".into());
        }
        redirect_policy(&url, &next, &mut headers)?;
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
    unreachable!("Every redirect loop exit returns a response or an error")
}

fn option_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(v) => v.clone(),
        v => v.to_string(),
    }
}

pub(crate) fn perform_xmlrpc_call(
    raw_url: &str,
    method_name: &str,
    arguments: &serde_json::Value,
    allowed_domains: &[String],
) -> Result<serde_json::Value, String> {
    let request_body = build_xmlrpc_request(method_name, arguments)?;
    if request_body.len() > MAX_NETWORK_REQUEST_BYTES {
        return Err("XML-RPC request exceeds the 1 MiB limit".into());
    }
    let mut headers = ureq::http::HeaderMap::new();
    headers.insert(
        ureq::http::header::CONTENT_TYPE,
        ureq::http::HeaderValue::from_static("text/xml; charset=utf-8"),
    );
    headers.insert(
        ureq::http::header::ACCEPT,
        ureq::http::HeaderValue::from_static("text/xml"),
    );
    let mut response = fetch(
        url::Url::parse(raw_url).map_err(|e| e.to_string())?,
        ureq::http::Method::POST,
        headers,
        request_body,
        allowed_domains,
    )?;
    let status = response.status();
    let body = response
        .body_mut()
        .with_config()
        .limit((MAX_XMLRPC_RESPONSE_BYTES + 1) as u64)
        .read_to_vec()
        .map_err(|error| format!("Could not read XML-RPC response: {error}"))?;
    if body.len() > MAX_XMLRPC_RESPONSE_BYTES {
        return Err("XML-RPC response exceeds the 4 MiB limit".into());
    }
    if !(200..300).contains(&status.as_u16()) {
        return Err(format!(
            "XML-RPC request failed with status {}",
            status.as_u16()
        ));
    }
    let document = roxmltree::Document::parse(
        std::str::from_utf8(&body)
            .map_err(|error| format!("XML-RPC response is not UTF-8: {error}"))?,
    )
    .map_err(|error| format!("Invalid XML-RPC response: {error}"))?;
    let root = document.root_element();
    if let Some(fault) = root.children().find(|node| node.has_tag_name("fault")) {
        let details = fault
            .descendants()
            .find(|node| node.has_tag_name("value"))
            .map(|value| parse_xmlrpc_value(value, 0))
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        return Err(format!("XML-RPC fault: {details}"));
    }
    let value = root
        .children()
        .find(|node| node.has_tag_name("params"))
        .and_then(|params| params.children().find(|node| node.has_tag_name("param")))
        .and_then(|param| param.children().find(|node| node.has_tag_name("value")))
        .ok_or_else(|| "XML-RPC server returned no value".to_owned())?;
    parse_xmlrpc_value(value, 0)
}

const MAX_XMLRPC_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

fn build_xmlrpc_request(
    method_name: &str,
    arguments: &serde_json::Value,
) -> Result<Vec<u8>, String> {
    use quick_xml::Writer;

    let mut writer = Writer::new(Vec::new());
    xmlrpc_start(&mut writer, "methodCall")?;
    xmlrpc_text_element(&mut writer, "methodName", method_name)?;
    xmlrpc_start(&mut writer, "params")?;
    for argument in arguments
        .as_array()
        .ok_or_else(|| "XML-RPC arguments must be an array".to_owned())?
    {
        xmlrpc_start(&mut writer, "param")?;
        write_xmlrpc_value(&mut writer, argument, 0)?;
        xmlrpc_end(&mut writer, "param")?;
    }
    xmlrpc_end(&mut writer, "params")?;
    xmlrpc_end(&mut writer, "methodCall")?;
    Ok(writer.into_inner())
}

fn xmlrpc_start(writer: &mut quick_xml::Writer<Vec<u8>>, name: &str) -> Result<(), String> {
    writer
        .write_event(quick_xml::events::Event::Start(
            quick_xml::events::BytesStart::new(name),
        ))
        .map_err(|error| error.to_string())
}

fn xmlrpc_end(writer: &mut quick_xml::Writer<Vec<u8>>, name: &str) -> Result<(), String> {
    writer
        .write_event(quick_xml::events::Event::End(
            quick_xml::events::BytesEnd::new(name),
        ))
        .map_err(|error| error.to_string())
}

fn xmlrpc_text_element(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    name: &str,
    text: &str,
) -> Result<(), String> {
    xmlrpc_start(writer, name)?;
    writer
        .write_event(quick_xml::events::Event::Text(
            quick_xml::events::BytesText::new(text),
        ))
        .map_err(|error| error.to_string())?;
    xmlrpc_end(writer, name)
}

fn write_xmlrpc_value(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    value: &serde_json::Value,
    depth: usize,
) -> Result<(), String> {
    use base64::Engine;
    use quick_xml::events::{BytesStart, Event};

    if depth > 64 {
        return Err("XML-RPC value nesting exceeds 64 levels".into());
    }
    xmlrpc_start(writer, "value")?;
    match value {
        serde_json::Value::Null => {
            writer
                .write_event(Event::Empty(BytesStart::new("nil")))
                .map_err(|error| error.to_string())?;
        }
        serde_json::Value::Bool(value) => {
            xmlrpc_text_element(writer, "boolean", if *value { "1" } else { "0" })?;
        }
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                xmlrpc_text_element(writer, "int", &value.to_string())?;
            } else if let Some(value) = value.as_u64() {
                xmlrpc_text_element(writer, "i8", &value.to_string())?;
            } else if let Some(value) = value.as_f64().filter(|value| value.is_finite()) {
                xmlrpc_text_element(writer, "double", &value.to_string())?;
            } else {
                return Err("XML-RPC numbers must be finite".into());
            }
        }
        serde_json::Value::String(value) => xmlrpc_text_element(writer, "string", value)?,
        serde_json::Value::Array(values) => {
            xmlrpc_start(writer, "array")?;
            xmlrpc_start(writer, "data")?;
            for value in values {
                write_xmlrpc_value(writer, value, depth + 1)?;
            }
            xmlrpc_end(writer, "data")?;
            xmlrpc_end(writer, "array")?;
        }
        serde_json::Value::Object(values) => {
            if let Some(date) = values
                .get("__riinaXmlrpcDate")
                .and_then(serde_json::Value::as_str)
            {
                let date = chrono::DateTime::parse_from_rfc3339(date)
                    .map_err(|error| format!("Invalid XML-RPC date value: {error}"))?
                    .with_timezone(&chrono::Local)
                    .format("%Y%m%dT%H:%M:%S")
                    .to_string();
                xmlrpc_text_element(writer, "dateTime.iso8601", &date)?;
            } else if let Some(bytes) = values
                .get("__riinaXmlrpcBinary")
                .and_then(serde_json::Value::as_array)
            {
                let bytes = bytes
                    .iter()
                    .map(|value| {
                        value
                            .as_u64()
                            .filter(|value| *value <= 255)
                            .map(|value| value as u8)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| "XML-RPC binary data must contain byte values".to_owned())?;
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                xmlrpc_text_element(writer, "base64", &encoded)?;
            } else {
                xmlrpc_start(writer, "struct")?;
                for (name, member) in values {
                    xmlrpc_start(writer, "member")?;
                    xmlrpc_text_element(writer, "name", name)?;
                    write_xmlrpc_value(writer, member, depth + 1)?;
                    xmlrpc_end(writer, "member")?;
                }
                xmlrpc_end(writer, "struct")?;
            }
        }
    }
    xmlrpc_end(writer, "value")
}

fn parse_xmlrpc_value(
    value: roxmltree::Node<'_, '_>,
    depth: usize,
) -> Result<serde_json::Value, String> {
    use base64::Engine;

    if depth > 64 {
        return Err("XML-RPC value nesting exceeds 64 levels".into());
    }
    let Some(typed) = value.children().find(|node| node.is_element()) else {
        return Ok(serde_json::Value::String(
            value.text().unwrap_or_default().to_owned(),
        ));
    };
    let text = typed.text().unwrap_or_default().trim();
    match typed.tag_name().name() {
        "string" => Ok(serde_json::Value::String(
            typed.text().unwrap_or_default().to_owned(),
        )),
        "int" | "i4" | "i8" => text
            .parse::<i64>()
            .map(serde_json::Value::from)
            .map_err(|error| format!("Invalid XML-RPC integer: {error}")),
        "double" => text
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "Invalid XML-RPC double".to_owned()),
        "boolean" => match text {
            "1" | "true" => Ok(serde_json::Value::Bool(true)),
            "0" | "false" => Ok(serde_json::Value::Bool(false)),
            _ => Err("Invalid XML-RPC boolean".into()),
        },
        "dateTime.iso8601" => {
            use chrono::TimeZone;
            let date = chrono::NaiveDateTime::parse_from_str(text, "%Y%m%dT%H:%M:%S")
                .map_err(|error| format!("Invalid XML-RPC date: {error}"))?;
            let date = match chrono::Local.from_local_datetime(&date) {
                chrono::LocalResult::Single(date) | chrono::LocalResult::Ambiguous(date, _) => date,
                chrono::LocalResult::None => {
                    return Err("XML-RPC date does not exist in the local time zone".into());
                }
            }
            .to_rfc3339();
            Ok(serde_json::json!({ "__riinaXmlrpcDate": date }))
        }
        "base64" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(text)
                .map_err(|error| format!("Invalid XML-RPC base64 data: {error}"))?;
            Ok(serde_json::json!({ "__riinaXmlrpcBinary": bytes }))
        }
        "nil" => Ok(serde_json::Value::Null),
        "array" => {
            let data = typed
                .children()
                .find(|node| node.has_tag_name("data"))
                .ok_or_else(|| "XML-RPC array has no data element".to_owned())?;
            data.children()
                .filter(|node| node.has_tag_name("value"))
                .map(|value| parse_xmlrpc_value(value, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(serde_json::Value::Array)
        }
        "struct" => {
            let mut members = serde_json::Map::new();
            for member in typed.children().filter(|node| node.has_tag_name("member")) {
                let name = member
                    .children()
                    .find(|node| node.has_tag_name("name"))
                    .and_then(|node| node.text())
                    .ok_or_else(|| "XML-RPC struct member has no name".to_owned())?;
                let value = member
                    .children()
                    .find(|node| node.has_tag_name("value"))
                    .ok_or_else(|| "XML-RPC struct member has no value".to_owned())?;
                members.insert(name.to_owned(), parse_xmlrpc_value(value, depth + 1)?);
            }
            Ok(serde_json::Value::Object(members))
        }
        other => Err(format!("Unsupported XML-RPC value type: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread::JoinHandle,
    };

    fn server(
        listener: TcpListener,
        count: usize,
        response: impl Fn(usize) -> String + Send + 'static,
    ) -> JoinHandle<Vec<String>> {
        listener.set_nonblocking(true).unwrap();
        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(4);
            let mut requests = Vec::new();
            for index in 0..count {
                let mut stream: TcpStream = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < until =>
                        {
                            std::thread::sleep(Duration::from_millis(2))
                        }
                        Err(e) => panic!("Server did not receive its request: {e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0];
                while !bytes.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let headers = String::from_utf8(bytes).unwrap();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                requests.push(headers.to_ascii_lowercase());
                stream.write_all(response(index).as_bytes()).unwrap();
            }
            requests
        })
    }
    fn ok() -> String {
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".into()
    }
    fn redirect(location: &str) -> String {
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    }
    fn run(download: bool, url: &str, options: &serde_json::Value) -> Result<(), String> {
        if download {
            perform_http_download(url, options, &["127.0.0.1".into()]).map(|_| ())
        } else {
            perform_http_request("GET", url, options, &["127.0.0.1".into()]).map(|_| ())
        }
    }
    #[test]
    fn request_and_download_strip_credentials_on_port_change() {
        for download in [false, true] {
            let first = TcpListener::bind("127.0.0.1:0").unwrap();
            let first_url = format!("http://{}/", first.local_addr().unwrap());
            let second = TcpListener::bind("127.0.0.1:0").unwrap();
            let target = format!("http://{}/target", second.local_addr().unwrap());
            let a = server(first, 1, move |_| redirect(&target));
            let b = server(second, 1, |_| ok());
            run(
                download,
                &first_url,
                &serde_json::json!({"headers":{"Authorization":"Bearer dummy","Cookie":"dummy=1"}}),
            )
            .unwrap();
            assert!(a.join().unwrap()[0].contains("authorization: bearer dummy"));
            let received = b.join().unwrap();
            assert!(!received[0].contains("authorization:"));
            assert!(!received[0].contains("cookie:"));
        }
    }
    #[test]
    fn same_origin_credentials_remain_and_redirect_exhaustion_fails() {
        for download in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/", listener.local_addr().unwrap());
            let worker = server(
                listener,
                2,
                |i| if i == 0 { redirect("/next") } else { ok() },
            );
            run(
                download,
                &url,
                &serde_json::json!({"headers":{"Authorization":"Bearer dummy"}}),
            )
            .unwrap();
            assert!(
                worker
                    .join()
                    .unwrap()
                    .iter()
                    .all(|r| r.contains("authorization: bearer dummy"))
            );
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/", listener.local_addr().unwrap());
            let worker = server(listener, 11, |_| redirect("/loop"));
            assert!(
                run(download, &url, &serde_json::json!({}))
                    .unwrap_err()
                    .contains("redirect limit")
            );
            assert_eq!(worker.join().unwrap().len(), 11);
        }
    }
    #[test]
    fn redirect_policy_checks_scheme_host_and_effective_port() {
        let url = url::Url::parse("https://example.org/").unwrap();
        for target in ["https://example.org:444/", "https://other.example.org/"] {
            let mut headers = ureq::http::HeaderMap::new();
            headers.insert(
                "authorization",
                ureq::http::HeaderValue::from_static("dummy"),
            );
            redirect_policy(&url, &url::Url::parse(target).unwrap(), &mut headers).unwrap();
            assert!(!headers.contains_key("authorization"));
        }
        let mut headers = ureq::http::HeaderMap::new();
        headers.insert(
            "authorization",
            ureq::http::HeaderValue::from_static("dummy"),
        );
        redirect_policy(
            &url,
            &url::Url::parse("https://example.org:443/").unwrap(),
            &mut headers,
        )
        .unwrap();
        assert!(headers.contains_key("authorization"));
        assert!(
            redirect_policy(
                &url,
                &url::Url::parse("http://example.org/").unwrap(),
                &mut headers
            )
            .is_err()
        );
    }
    #[test]
    fn post_redirect_discards_body_headers_and_blocked_targets_never_connect() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = server(
            listener,
            2,
            |i| if i == 0 { redirect("/next") } else { ok() },
        );
        perform_http_request(
            "POST",
            &url,
            &serde_json::json!({"data":{"x":"value"},"headers":{"Content-Encoding":"identity"}}),
            &["127.0.0.1".into()],
        )
        .unwrap();
        let received = worker.join().unwrap();
        assert!(received[0].starts_with("post "));
        assert!(received[1].starts_with("get "));
        assert!(!received[1].contains("content-type:"));
        assert!(!received[1].contains("content-encoding:"));
        for download in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/", listener.local_addr().unwrap());
            let worker = server(listener, 1, |_| redirect("http://unapproved.invalid/"));
            assert!(
                run(download, &url, &serde_json::json!({}))
                    .unwrap_err()
                    .contains("allowed domains")
            );
            worker.join().unwrap();
        }
    }
}
