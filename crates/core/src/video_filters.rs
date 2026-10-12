//! User-saved audio and video filters and their observed mpv chains.
use crate::crop::VideoFilter;
use serde::{Deserialize, Serialize};

pub const PRESET_LABELS: [&str; 11] = [
    "Crop",
    "Expand",
    "Sharpen",
    "Blur",
    "Delogo",
    "Negative",
    "Flip",
    "Mirror",
    "3D LUT",
    "Custom (mpv)",
    "Custom (FFmpeg)",
];
pub const AUDIO_PRESET_LABELS: [&str; 2] = ["Custom (mpv)", "Custom (FFmpeg)"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Text,
    Integer,
    Float,
    Choose,
}

#[derive(Debug, Clone, Copy)]
pub struct ParameterSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub kind: ParameterKind,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub choices: &'static [&'static str],
}

const fn text(name: &'static str, label: &'static str, default: &'static str) -> ParameterSpec {
    ParameterSpec {
        name,
        label,
        default,
        kind: ParameterKind::Text,
        minimum: 0.0,
        maximum: 0.0,
        step: 0.0,
        choices: &[],
    }
}
const fn integer(
    name: &'static str,
    label: &'static str,
    default: &'static str,
    minimum: f64,
    maximum: f64,
    step: f64,
) -> ParameterSpec {
    ParameterSpec {
        name,
        label,
        default,
        kind: ParameterKind::Integer,
        minimum,
        maximum,
        step,
        choices: &[],
    }
}
const fn float(
    name: &'static str,
    label: &'static str,
    default: &'static str,
    minimum: f64,
    maximum: f64,
    step: f64,
) -> ParameterSpec {
    ParameterSpec {
        name,
        label,
        default,
        kind: ParameterKind::Float,
        minimum,
        maximum,
        step,
        choices: &[],
    }
}
const fn choose(
    name: &'static str,
    label: &'static str,
    default: &'static str,
    choices: &'static [&'static str],
) -> ParameterSpec {
    ParameterSpec {
        name,
        label,
        default,
        kind: ParameterKind::Choose,
        minimum: 0.0,
        maximum: 0.0,
        step: 0.0,
        choices,
    }
}

const CROP: [ParameterSpec; 4] = [
    text("w", "Width (default: video width)", ""),
    text("h", "Height (default: video height)", ""),
    text("x", "Origin X (default: center)", ""),
    text("y", "Origin Y (default: center)", ""),
];
const EXPAND: [ParameterSpec; 6] = [
    text("w", "Width (default: video width)", ""),
    text("h", "Height (default: video height)", ""),
    text("x", "Origin X (default: center)", ""),
    text("y", "Origin Y (default: center)", ""),
    text("aspect", "Aspect ratio (e.g. 4/3)", "0"),
    text("round", "Round up to a multiple of", "1"),
];
const SHARPEN: [ParameterSpec; 2] = [
    integer("msize", "Matrix size", "5", 3.0, 13.0, 2.0),
    float("amount", "Amount", "0", 0.0, 1.5, 0.01),
];
const DELOGO: [ParameterSpec; 4] = [
    text("x", "Origin X", "1"),
    text("y", "Origin Y", "1"),
    text("w", "Width", "1"),
    text("h", "Height", "1"),
];
const LUT3D: [ParameterSpec; 2] = [
    text("file", "File path", ""),
    choose(
        "interp",
        "Interpolation mode",
        "nearest",
        &["nearest", "trilinear", "tetrahedral"],
    ),
];
const CUSTOM: [ParameterSpec; 2] = [
    text("name", "Filter name", ""),
    text("string", "Filter value", ""),
];

pub fn preset_parameters(index: usize) -> Option<&'static [ParameterSpec]> {
    Some(match index {
        0 => &CROP,
        1 => &EXPAND,
        2 | 3 => &SHARPEN,
        4 => &DELOGO,
        5..=7 => &[],
        8 => &LUT3D,
        9..=10 => &CUSTOM,
        _ => return None,
    })
}

pub fn audio_preset_parameters(index: usize) -> Option<&'static [ParameterSpec]> {
    (index < AUDIO_PRESET_LABELS.len()).then_some(&CUSTOM)
}

pub fn default_parameter_values(index: usize) -> Option<Vec<String>> {
    Some(
        preset_parameters(index)?
            .iter()
            .map(|parameter| parameter.default.to_owned())
            .collect(),
    )
}

pub fn default_audio_parameter_values(index: usize) -> Option<Vec<String>> {
    Some(
        audio_preset_parameters(index)?
            .iter()
            .map(|parameter| parameter.default.to_owned())
            .collect(),
    )
}

pub fn build_preset_filter(index: usize, values: &[String]) -> Result<String, &'static str> {
    let parameters = preset_parameters(index).ok_or("Unknown video filter preset")?;
    if parameters.len() != values.len() {
        return Err("The video filter preset parameter count is invalid");
    }
    for (parameter, value) in parameters.iter().zip(values) {
        if value.chars().any(char::is_control) {
            return Err("Video filter parameters cannot contain control characters");
        }
        match parameter.kind {
            ParameterKind::Integer => {
                let value = value
                    .parse::<i64>()
                    .map_err(|_| "The filter parameter must be an integer")?;
                let value = value as f64;
                if value < parameter.minimum
                    || value > parameter.maximum
                    || (value - parameter.minimum) % parameter.step != 0.0
                {
                    return Err("The integer filter parameter is outside its supported steps");
                }
            }
            ParameterKind::Float => {
                let value = value
                    .parse::<f64>()
                    .map_err(|_| "The filter parameter must be a number")?;
                if !value.is_finite() || value < parameter.minimum || value > parameter.maximum {
                    return Err("The numeric filter parameter is outside its supported range");
                }
            }
            ParameterKind::Choose if !parameter.choices.contains(&value.as_str()) => {
                return Err("The filter parameter choice is invalid");
            }
            ParameterKind::Text | ParameterKind::Choose => {}
        }
    }
    let value = |name: &str| -> &str {
        parameters
            .iter()
            .position(|parameter| parameter.name == name)
            .map_or("", |index| values[index].as_str())
    };
    if index == 8 && value("file").trim().is_empty() {
        return Err("Choose a 3D LUT file");
    }
    if matches!(index, 9..=10) && !valid_filter_name(value("name")) {
        return Err("The filter name is empty or invalid");
    }
    let filter = match index {
        0 => format!(
            "crop={}:{}:{}:{}",
            value("w"),
            value("h"),
            value("x"),
            value("y")
        ),
        1 => format!(
            "expand={}:{}:{}:{}:{}:{}",
            value("w"),
            value("h"),
            value("x"),
            value("y"),
            value("aspect"),
            value("round")
        ),
        2 | 3 => {
            let size = value("msize");
            let amount = value("amount")
                .parse::<f64>()
                .map_err(|_| "The sharpen amount must be a number")?;
            let amount = if index == 3 { -amount } else { amount };
            format!("lavfi=[unsharp={size}:{size}:{amount}:{size}:{size}:{amount}]")
        }
        4 => format!(
            "delogo=x={}:y={}:w={}:h={}",
            value("x"),
            value("y"),
            value("w"),
            value("h")
        ),
        5 => "lavfi=[lutrgb=r=negval:g=negval:b=negval]".to_owned(),
        6 => "vflip".to_owned(),
        7 => "hflip".to_owned(),
        // IINA puts the path verbatim inside the already bracketed lavfi graph.
        8 => format!(
            "lavfi=[lut3d=file={}:interp={}]",
            value("file"),
            value("interp")
        ),
        9 => format!("{}={}", value("name"), value("string")),
        10 => format!("lavfi=[{}={}]", value("name"), value("string")),
        _ => return Err("Unknown video filter preset"),
    };
    if !valid_filter_string(&filter) {
        return Err("The video filter preset is missing required values or is invalid");
    }
    Ok(filter)
}

pub fn build_audio_preset_filter(index: usize, values: &[String]) -> Result<String, &'static str> {
    if audio_preset_parameters(index).is_none() {
        return Err("Unknown audio filter preset");
    }
    build_preset_filter(index + 9, values)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedFilter {
    pub name: String,
    pub filter: String,
    #[serde(default)]
    pub shortcut_key: String,
    #[serde(default)]
    pub shortcut_modifiers: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub saved: Vec<SavedFilter>,
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.saved.len() > 128
            || self.saved.iter().any(|filter| {
                !valid_name(&filter.name)
                    || !valid_filter_string(&filter.filter)
                    || !valid_shortcut(&filter.shortcut_key, filter.shortcut_modifiers)
            })
        {
            return Err("Saved filters contain an invalid name or filter string");
        }
        for (index, filter) in self.saved.iter().enumerate() {
            if self.saved[..index]
                .iter()
                .any(|previous| previous.name == filter.name)
            {
                return Err("Saved filter names must be unique");
            }
        }
        Ok(())
    }

    pub fn save(&mut self, name: &str, filter: &str) -> Result<(), &'static str> {
        self.save_with_shortcut(name, filter, "", 0)
    }

    pub fn save_with_shortcut(
        &mut self,
        name: &str,
        filter: &str,
        shortcut_key: &str,
        shortcut_modifiers: u8,
    ) -> Result<(), &'static str> {
        let filter = SavedFilter {
            name: name.trim().to_owned(),
            filter: filter.trim().to_owned(),
            shortcut_key: shortcut_key.to_owned(),
            shortcut_modifiers,
        };
        if !valid_name(&filter.name) {
            return Err("Filter names must be 1 to 64 printable characters");
        }
        if !valid_filter_string(&filter.filter) {
            return Err("The filter string is empty or invalid");
        }
        if !valid_shortcut(&filter.shortcut_key, filter.shortcut_modifiers) {
            return Err("The saved-filter shortcut is invalid");
        }
        if self.saved.iter().any(|saved| saved.name == filter.name) {
            return Err("A saved filter already has that name");
        }
        if self.saved.len() == 128 {
            return Err("No more than 128 filters can be saved");
        }
        self.saved.push(filter);
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<SavedFilter, &'static str> {
        if index >= self.saved.len() {
            return Err("The selected saved filter no longer exists");
        }
        Ok(self.saved.remove(index))
    }

    pub fn update(&mut self, index: usize, name: &str, filter: &str) -> Result<(), &'static str> {
        let (shortcut_key, shortcut_modifiers) = self
            .saved
            .get(index)
            .map(|saved| (saved.shortcut_key.clone(), saved.shortcut_modifiers))
            .ok_or("The selected saved filter no longer exists")?;
        self.update_with_shortcut(index, name, filter, &shortcut_key, shortcut_modifiers)
    }

    pub fn update_with_shortcut(
        &mut self,
        index: usize,
        name: &str,
        filter: &str,
        shortcut_key: &str,
        shortcut_modifiers: u8,
    ) -> Result<(), &'static str> {
        let name = name.trim();
        let filter = filter.trim();
        if index >= self.saved.len() {
            return Err("The selected saved filter no longer exists");
        }
        if !valid_name(name) {
            return Err("Filter names must be 1 to 64 printable characters");
        }
        if !valid_filter_string(filter) {
            return Err("The filter string is empty or invalid");
        }
        if !valid_shortcut(shortcut_key, shortcut_modifiers) {
            return Err("The saved-filter shortcut is invalid");
        }
        if self
            .saved
            .iter()
            .enumerate()
            .any(|(current, saved)| current != index && saved.name == name)
        {
            return Err("A saved filter already has that name");
        }
        self.saved[index] = SavedFilter {
            name: name.to_owned(),
            filter: filter.to_owned(),
            shortcut_key: shortcut_key.to_owned(),
            shortcut_modifiers,
        };
        Ok(())
    }
}

pub const SHORTCUT_CONTROL: u8 = 1;
pub const SHORTCUT_ALT: u8 = 2;
pub const SHORTCUT_SHIFT: u8 = 4;
pub const SHORTCUT_META: u8 = 8;

fn valid_shortcut(key: &str, modifiers: u8) -> bool {
    key.len() <= 32
        && !key.chars().any(char::is_control)
        && modifiers & !(SHORTCUT_CONTROL | SHORTCUT_ALT | SHORTCUT_SHIFT | SHORTCUT_META) == 0
        && (!key.is_empty() || modifiers == 0)
}

pub fn valid_filter_string(filter: &str) -> bool {
    !filter.trim().is_empty()
        && filter.len() <= 4096
        && !filter.chars().any(|character| character.is_control())
}

pub fn format_filter(filter: &VideoFilter) -> String {
    let mut formatted = String::new();
    if let Some(label) = filter.label.as_deref() {
        formatted.push('@');
        formatted.push_str(label);
        formatted.push(':');
    }
    if filter.enabled == Some(false) {
        formatted.push('!');
    }
    formatted.push_str(&filter.name);
    if filter.name == "lavfi"
        && let Some(graph) = filter.params.get("graph")
    {
        formatted.push('=');
        formatted.push_str(graph);
        return formatted;
    }
    if filter.params.is_empty() {
        return formatted;
    }
    formatted.push('=');
    if filter.params.keys().all(|key| key.starts_with('@')) {
        let mut values = filter
            .params
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix('@')?
                    .parse::<usize>()
                    .ok()
                    .map(|i| (i, value))
            })
            .collect::<Vec<_>>();
        values.sort_by_key(|(index, _)| *index);
        formatted.push_str(
            &values
                .into_iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>()
                .join(":"),
        );
    } else {
        formatted.push_str(
            &filter
                .params
                .iter()
                .map(|(key, value)| format!("{key}={}", quote_value(value)))
                .collect::<Vec<_>>()
                .join(":"),
        );
    }
    formatted
}

fn quote_value(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        value.to_owned()
    } else {
        format!("[{value}]")
    }
}

fn valid_filter_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.chars().count() <= 64
        && !name.chars().any(|character| character.is_control())
}

/// Mirrors mpv's filter identity rules for menu checkmarks: a label is the
/// identity when present; otherwise the complete name and ordered arguments.
pub fn is_active(filter: &str, active: &[VideoFilter]) -> bool {
    let Some((label, name, arguments)) = parse_filter_identity(filter) else {
        return false;
    };
    active.iter().any(|candidate| {
        if candidate.name != name {
            return false;
        }
        if let Some(label) = label.as_deref() {
            return candidate.label.as_deref() == Some(label);
        }
        if candidate.label.is_some() {
            return false;
        }
        match arguments.as_deref() {
            None => candidate.params.is_empty(),
            Some(arguments) if name == "lavfi" => candidate
                .params
                .get("graph")
                .is_some_and(|graph| graph == arguments),
            Some(arguments) => {
                let named = arguments
                    .split(':')
                    .map(|part| part.split_once('='))
                    .collect::<Option<Vec<_>>>();
                if let Some(named) = named {
                    return named.len() == candidate.params.len()
                        && named.iter().all(|(key, value)| {
                            candidate
                                .params
                                .get(*key)
                                .is_some_and(|actual| actual.as_str() == *value)
                        });
                }
                let values = arguments.split(':').collect::<Vec<_>>();
                let indexed = values.iter().enumerate().all(|(index, value)| {
                    candidate
                        .params
                        .get(&format!("@{index}"))
                        .is_some_and(|actual| actual == value)
                });
                indexed && values.len() == candidate.params.len()
            }
        }
    })
}

fn parse_filter_identity(filter: &str) -> Option<(Option<String>, String, Option<String>)> {
    if !valid_filter_string(filter) {
        return None;
    }
    let (head, arguments) = filter
        .split_once('=')
        .map_or((filter, None), |(head, arguments)| (head, Some(arguments)));
    let (label, name) = if let Some(labelled) = head.strip_prefix('@') {
        let (label, name) = labelled.split_once(':')?;
        if label.is_empty() || name.is_empty() {
            return None;
        }
        (Some(label.to_owned()), name)
    } else {
        (None, head)
    };
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return None;
    }
    Some((label, name.to_owned(), arguments.map(str::to_owned)))
}
