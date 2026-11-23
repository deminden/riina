//! Decode IINA's package property lists into the shared preferences store.
use super::{MAX_PREFERENCE_BYTES, Values, encode};
use plist::{
    Value,
    stream::{Event, Reader},
};
use serde_json::{Number, Value as Json};
use std::io::Cursor;

pub(super) fn decode(bytes: &[u8]) -> Result<Values, String> {
    const MAX_DEPTH: usize = 120;
    let mut depth = 0usize;
    let mut remaining = 2 * MAX_PREFERENCE_BYTES;
    let mut exceeded = false;
    // Binary plists may reference the same object repeatedly. Limit the
    // expanded stream before it allocates a tree, as well as the file size.
    let events = Reader::new(Cursor::new(bytes)).map_while(|mut result| {
        if let Ok(event) = &mut result {
            let payload = match event {
                Event::StartDictionary(_) => {
                    depth += 1;
                    0
                }
                Event::StartArray(length) => {
                    *length = None;
                    depth += 1;
                    0
                }
                Event::EndCollection => {
                    depth = depth.saturating_sub(1);
                    0
                }
                Event::String(text) => text.len(),
                Event::Data(bytes) => bytes.len() * 2,
                _ => 0,
            };
            let cost = payload + 4;
            if depth > MAX_DEPTH || cost > remaining {
                exceeded = true;
                return None;
            }
            remaining -= cost;
        }
        Some(result)
    });
    let parsed = Value::from_events(events);
    if exceeded {
        return Err("IINA plugin preferences are too deeply nested or too large".into());
    }
    let parsed =
        parsed.map_err(|error| format!("Could not read IINA plugin preferences: {error}"))?;
    let Json::Object(values) = convert(parsed)? else {
        return Err("IINA plugin preferences must contain a dictionary".into());
    };
    encode(&values)?;
    Ok(values)
}

fn convert(value: Value) -> Result<Json, String> {
    Ok(match value {
        Value::Array(values) => Json::Array(
            values
                .into_iter()
                .map(convert)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Dictionary(values) => Json::Object(
            values
                .into_iter()
                .map(|(key, value)| convert(value).map(|value| (key, value)))
                .collect::<Result<Values, _>>()?,
        ),
        Value::Boolean(value) => Json::Bool(value),
        Value::Data(bytes) => Json::Array(bytes.into_iter().map(Json::from).collect()),
        Value::Date(date) => {
            let time = chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::from(date));
            Json::String(time.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true))
        }
        Value::Integer(value) => Json::Number(
            value
                .as_signed()
                .map(Number::from)
                .or_else(|| value.as_unsigned().map(Number::from))
                .ok_or("IINA plugin preferences contain an unsupported integer")?,
        ),
        Value::Real(value) => Json::Number(
            Number::from_f64(value).ok_or("IINA plugin preferences contain a nonfinite number")?,
        ),
        Value::String(value) => Json::String(value),
        _ => {
            return Err(
                "IINA plugin preferences contain an unsupported property-list value".into(),
            );
        }
    })
}
