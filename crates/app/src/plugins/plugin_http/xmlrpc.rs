use super::{MAX_NETWORK_REQUEST_BYTES, fetch};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const VALUE_TAG: &str = "__riinaXmlrpcType";
const MAX_XMLRPC_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Serialize)]
#[serde(tag = "state", content = "data", rename_all = "snake_case")]
pub(crate) enum XmlrpcReply {
    Value(Value),
    Fault,
    Failed(XmlrpcFailure),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XmlrpcFailure {
    http_code: u16,
    reason: String,
    description: String,
}

impl XmlrpcFailure {
    fn new(method: &str, http_code: u16, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            http_code,
            description: format!("{method}: [{http_code}] {reason}"),
            reason,
        }
    }
}

pub(crate) fn perform_xmlrpc_call(
    raw_url: &str,
    method_name: &str,
    arguments: &[Value],
    allowed_domains: &[String],
) -> XmlrpcReply {
    request_xmlrpc(raw_url, method_name, arguments, allowed_domains)
        .unwrap_or_else(XmlrpcReply::Failed)
}

fn request_xmlrpc(
    raw_url: &str,
    method_name: &str,
    arguments: &[Value],
    allowed_domains: &[String],
) -> Result<XmlrpcReply, XmlrpcFailure> {
    let request_error = |reason: String| XmlrpcFailure::new(method_name, 0, reason);
    let request_body = build_xmlrpc_request(method_name, arguments).map_err(request_error)?;
    if request_body.len() > MAX_NETWORK_REQUEST_BYTES {
        return Err(request_error(
            "XML-RPC request exceeds the 1 MiB limit".into(),
        ));
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
        url::Url::parse(raw_url).map_err(|error| request_error(error.to_string()))?,
        ureq::http::Method::POST,
        headers,
        request_body,
        allowed_domains,
    )
    .map_err(request_error)?;
    let status = response.status();
    let http_code = status.as_u16();
    if !status.is_success() {
        let reason = status.canonical_reason().unwrap_or("HTTP error");
        return Err(XmlrpcFailure::new(method_name, http_code, reason));
    }
    let bad_response = || XmlrpcFailure::new(method_name, http_code, "Bad response");
    let body = response
        .body_mut()
        .with_config()
        .limit((MAX_XMLRPC_RESPONSE_BYTES + 1) as u64)
        .read_to_vec()
        .map_err(|_| bad_response())?;
    if body.len() > MAX_XMLRPC_RESPONSE_BYTES {
        return Err(bad_response());
    }
    parse_response(&body).map_err(|_| bad_response())
}

fn parse_response(body: &[u8]) -> Result<XmlrpcReply, String> {
    let xml = std::str::from_utf8(body).map_err(|error| error.to_string())?;
    let document = roxmltree::Document::parse(xml).map_err(|error| error.to_string())?;
    let root = document.root_element();
    if !root.has_tag_name("methodResponse") {
        return Err("Expected an XML-RPC methodResponse".into());
    }
    let result = only_child(root)?;
    if result.has_tag_name("fault") {
        return Ok(XmlrpcReply::Fault);
    }
    let param = only_child(result)?;
    let value = only_child(param)?;
    if !result.has_tag_name("params")
        || !param.has_tag_name("param")
        || !value.has_tag_name("value")
    {
        return Err("Expected one XML-RPC return value".into());
    }
    parse_xmlrpc_value(value, 0).map(XmlrpcReply::Value)
}

fn only_child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
) -> Result<roxmltree::Node<'a, 'input>, String> {
    let mut children = node.children().filter(roxmltree::Node::is_element);
    match (children.next(), children.next()) {
        (Some(child), None) => Ok(child),
        _ => Err("Expected exactly one XML-RPC element".into()),
    }
}

fn build_xmlrpc_request(method_name: &str, arguments: &[Value]) -> Result<Vec<u8>, String> {
    use quick_xml::Writer;

    let mut writer = Writer::new(Vec::new());
    xmlrpc_start(&mut writer, "methodCall")?;
    xmlrpc_text_element(&mut writer, "methodName", method_name)?;
    xmlrpc_start(&mut writer, "params")?;
    for argument in arguments {
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
    if !text.chars().all(|c| {
        matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
    }) {
        return Err("XML-RPC text contains a character outside XML 1.0".into());
    }
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
        serde_json::Value::Object(values) => write_object(writer, values, depth)?,
    }
    xmlrpc_end(writer, "value")
}

fn write_object(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    values: &Map<String, Value>,
    depth: usize,
) -> Result<(), String> {
    use base64::{Engine, engine::general_purpose::STANDARD};

    let data = values.get("value").unwrap_or(&Value::Null);
    let members = match values.get(VALUE_TAG).and_then(Value::as_str) {
        Some("date") => {
            let date = data.as_str().ok_or("Invalid XML-RPC date value")?;
            let date = chrono::DateTime::parse_from_rfc3339(date)
                .map_err(|error| format!("Invalid XML-RPC date value: {error}"))?
                .with_timezone(&chrono::Local)
                .format("%Y%m%dT%H:%M:%S")
                .to_string();
            return xmlrpc_text_element(writer, "dateTime.iso8601", &date);
        }
        Some("binary") => {
            let bytes = Vec::<u8>::deserialize(data)
                .map_err(|_| "XML-RPC binary data must contain byte values")?;
            return xmlrpc_text_element(writer, "base64", &STANDARD.encode(bytes));
        }
        Some("struct") => data.as_object().ok_or("Invalid XML-RPC struct")?,
        _ => values,
    };
    xmlrpc_start(writer, "struct")?;
    for (name, value) in members {
        xmlrpc_start(writer, "member")?;
        xmlrpc_text_element(writer, "name", name)?;
        write_xmlrpc_value(writer, value, depth + 1)?;
        xmlrpc_end(writer, "member")?;
    }
    xmlrpc_end(writer, "struct")
}

fn parse_xmlrpc_value(
    value: roxmltree::Node<'_, '_>,
    depth: usize,
) -> Result<serde_json::Value, String> {
    use base64::Engine;

    if depth > 64 {
        return Err("XML-RPC value nesting exceeds 64 levels".into());
    }
    let mut types = value.children().filter(roxmltree::Node::is_element);
    let Some(typed) = types.next() else {
        return Ok(Value::String(text_content(value)));
    };
    if types.next().is_some() {
        return Err("XML-RPC value contains multiple types".into());
    }
    let contents = text_content(typed);
    let text = contents.trim();
    match typed.tag_name().name() {
        "string" => Ok(Value::String(contents)),
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
            Ok(serde_json::json!({ VALUE_TAG: "date", "value": date }))
        }
        "base64" => {
            let compact: Vec<u8> = text
                .bytes()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(compact)
                .map_err(|error| format!("Invalid XML-RPC base64 data: {error}"))?;
            Ok(serde_json::json!({ VALUE_TAG: "binary", "value": bytes }))
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
                    .map(text_content)
                    .ok_or_else(|| "XML-RPC struct member has no name".to_owned())?;
                let value = member
                    .children()
                    .find(|node| node.has_tag_name("value"))
                    .ok_or_else(|| "XML-RPC struct member has no value".to_owned())?;
                members.insert(name, parse_xmlrpc_value(value, depth + 1)?);
            }
            if members.contains_key(VALUE_TAG) {
                Ok(serde_json::json!({ VALUE_TAG: "struct", "value": members }))
            } else {
                Ok(Value::Object(members))
            }
        }
        other => Err(format!("Unsupported XML-RPC value type: {other}")),
    }
}

fn text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.children()
        .filter(roxmltree::Node::is_text)
        .filter_map(|child| child.text())
        .collect()
}
