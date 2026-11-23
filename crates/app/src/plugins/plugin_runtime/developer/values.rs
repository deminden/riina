//! Capture shallow previews on the owning worker while its execution budget is active.
use super::ExecutionControl;
use rquickjs::{CatchResultExt, Coerced, FromJs, Object, Type, Value, object::Filter};

const MAX_MEMBERS: usize = 100;
const MAX_MEMBER_CHARS: usize = 160;

pub(crate) struct Member {
    pub(crate) key: String,
    pub(crate) value: String,
}

pub(super) struct Preview {
    pub(super) summary: String,
    pub(super) members: Option<Vec<Member>>,
}

pub(super) fn inspect(value: Value<'_>, execution: &ExecutionControl) -> rquickjs::Result<Preview> {
    if let Some(array) = value.as_array() {
        // Large sparse arrays can have lengths beyond QuickJS's immediate integer range.
        let len = array.as_object().get::<_, u32>("length")? as usize;
        let members = (0..len.min(MAX_MEMBERS))
            .map(|index| Ok(member(index.to_string(), array.get(index), value.ctx())));
        return collect(format!("Array ({len})"), len, members, execution);
    }
    if !value.is_function()
        && let Some(object) = value.as_object()
    {
        return properties(object, Filter::default(), "Object", execution);
    }
    Ok(Preview {
        summary: summary(value)?,
        members: None,
    })
}

pub(super) fn globals(
    object: Object<'_>,
    execution: &ExecutionControl,
) -> rquickjs::Result<Preview> {
    properties(&object, Filter::new().string(), "Global Objects", execution)
}

fn properties<'js>(
    object: &Object<'js>,
    filter: Filter,
    title: &str,
    execution: &ExecutionControl,
) -> rquickjs::Result<Preview> {
    let keys = object.own_keys::<String>(filter);
    let len = keys.len();
    let members = keys.take(MAX_MEMBERS).map(|key| {
        let key = key?;
        let value = object.get(key.as_str());
        Ok(member(key, value, object.ctx()))
    });
    collect(title.into(), len, members, execution)
}

fn collect(
    summary: String,
    len: usize,
    members: impl Iterator<Item = rquickjs::Result<Member>>,
    execution: &ExecutionControl,
) -> rquickjs::Result<Preview> {
    let mut captured = Vec::with_capacity(len.min(MAX_MEMBERS));
    for item in members {
        captured.push(item?);
        if execution.interrupted() {
            break;
        }
    }
    if len > captured.len() {
        captured.push(Member {
            key: String::new(),
            value: format!("… ({} more)", len - captured.len()),
        });
    }
    Ok(Preview {
        summary,
        members: Some(captured),
    })
}

fn member<'js>(
    key: String,
    value: rquickjs::Result<Value<'js>>,
    ctx: &rquickjs::Ctx<'js>,
) -> Member {
    let value = value
        .and_then(summary)
        .catch(ctx)
        .unwrap_or_else(|_| "[Unavailable]".into());
    Member {
        key: if key.is_empty() {
            "\"\"".into()
        } else {
            member_text(&key)
        },
        value: member_text(&value),
    }
}

fn summary(value: Value<'_>) -> rquickjs::Result<String> {
    match value.type_of() {
        Type::Array => Ok("[Array]".into()),
        Type::Function | Type::Constructor => Ok("[Function]".into()),
        Type::Object | Type::Proxy => Ok("[Object]".into()),
        Type::Promise => Ok("[Promise]".into()),
        Type::Exception => Ok("[Error]".into()),
        Type::Symbol => Ok("[Symbol]".into()),
        _ => Coerced::<String>::from_js(&value.ctx().clone(), value).map(|text| text.0),
    }
}

fn member_text(text: &str) -> String {
    abbreviate(text, MAX_MEMBER_CHARS).replace(['\r', '\n', '\t'], " ")
}

pub(super) fn abbreviate(text: &str, max_chars: usize) -> String {
    let Some((end, _)) = text.char_indices().nth(max_chars) else {
        return text.into();
    };
    format!("{}…", &text[..end])
}
