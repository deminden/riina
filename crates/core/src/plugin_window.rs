//! Portable state of a realm-owned standalone WebView window.
use crate::plugin::{
    validate_message_data, validate_message_name, validate_overlay_text,
    validate_plugin_relative_path,
};
use serde::Deserialize;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

const MAX_QUEUED_MESSAGES: usize = 128;
const MAX_QUEUED_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Open,
    Close,
    LoadFile(PathBuf),
    SimpleMode,
    SetStyle(String),
    SetContent(String),
    SetProperties(Properties),
    SetFrame(Frame),
    PostMessage(String, Value),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Properties {
    pub title: Option<String>,
    pub resizable: Option<bool>,
    pub full_size_content_view: Option<bool>,
    pub hide_title_bar: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Frame {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Document {
    #[default]
    Blank,
    Package(PathBuf),
    Simple {
        style: Option<Arc<str>>,
        content: Option<Arc<str>>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Presentation {
    pub revision: u64,
    pub document_id: u64,
    pub visible: bool,
    pub document: Document,
    pub properties: Properties,
    pub frame: Frame,
}

#[derive(Default)]
pub struct State {
    presentation: Presentation,
    created: bool,
    messages: Vec<(String, Value)>,
    message_bytes: usize,
}

impl State {
    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    pub fn apply(&mut self, intent: Intent) -> Result<(), String> {
        match intent {
            Intent::Open | Intent::Close => {
                self.created = true;
                self.presentation.visible = matches!(intent, Intent::Open);
            }
            Intent::LoadFile(path) => {
                validate_plugin_relative_path(&path)
                    .map_err(|_| "Expected a safe relative plugin path")?;
                self.replace_document(Document::Package(path));
            }
            Intent::SimpleMode => self.replace_document(Document::Simple {
                style: None,
                content: None,
            }),
            Intent::SetStyle(value) => self.update_simple(true, value)?,
            Intent::SetContent(value) => self.update_simple(false, value)?,
            Intent::SetProperties(overrides) if self.created => {
                self.presentation.properties.merge(overrides)?;
            }
            Intent::SetProperties(_) => return Ok(()),
            Intent::SetFrame(frame) => {
                self.presentation.frame.merge(frame)?;
                self.created = true;
            }
            Intent::PostMessage(name, data) => {
                if self.created {
                    self.queue_message(name, data)?;
                }
                return Ok(());
            }
        }
        self.presentation.revision = self.presentation.revision.wrapping_add(1);
        Ok(())
    }

    pub fn take_messages(&mut self, document_id: u64) -> Vec<(String, Value)> {
        if self.presentation.document_id != document_id {
            return Vec::new();
        }
        self.message_bytes = 0;
        std::mem::take(&mut self.messages)
    }

    fn replace_document(&mut self, document: Document) {
        self.created = true;
        self.presentation.document_id = self.presentation.document_id.wrapping_add(1);
        self.presentation.document = document;
        self.messages.clear();
        self.message_bytes = 0;
    }

    fn update_simple(&mut self, is_style: bool, value: String) -> Result<(), String> {
        let Document::Simple { style, content } = &mut self.presentation.document else {
            return Err("Style and content require standalone simple mode".into());
        };
        validate_overlay_text(&value).map_err(|_| "Standalone style or content exceeds 1 MiB")?;
        let slot = if is_style { style } else { content };
        *slot = Some(value.into());
        Ok(())
    }

    fn queue_message(&mut self, name: String, data: Value) -> Result<(), String> {
        validate_message(&name, &data)?;
        let bytes = name.len()
            + serde_json::to_vec(&data)
                .map_err(|error| error.to_string())?
                .len();
        if self.messages.len() >= MAX_QUEUED_MESSAGES
            || self.message_bytes.saturating_add(bytes) > MAX_QUEUED_BYTES
        {
            return Err("The standalone window message queue is full".into());
        }
        self.message_bytes += bytes;
        self.messages.push((name, data));
        Ok(())
    }
}

impl Properties {
    fn merge(&mut self, pending: Self) -> Result<(), String> {
        if pending
            .title
            .as_ref()
            .is_some_and(|value| value.len() > 1024 || value.contains('\0'))
        {
            return Err("The standalone window title is invalid".into());
        }
        self.title = pending.title.or(self.title.take());
        self.resizable = pending.resizable.or(self.resizable);
        self.full_size_content_view = pending
            .full_size_content_view
            .or(self.full_size_content_view);
        self.hide_title_bar = pending.hide_title_bar.or(self.hide_title_bar);
        Ok(())
    }
}

impl Frame {
    fn merge(&mut self, pending: Self) -> Result<(), String> {
        if [pending.width, pending.height]
            .into_iter()
            .flatten()
            .any(|value| !(1.0..=8192.0).contains(&value))
            || [pending.x, pending.y]
                .into_iter()
                .flatten()
                .any(|value| !(-100_000.0..=100_000.0).contains(&value))
        {
            return Err("The standalone window frame is outside the supported range".into());
        }
        self.width = pending.width.or(self.width);
        self.height = pending.height.or(self.height);
        self.x = pending.x.or(self.x);
        self.y = pending.y.or(self.y);
        Ok(())
    }
}

pub fn validate_message(name: &str, data: &Value) -> Result<(), String> {
    validate_message_name(name).map_err(|_| "Expected a message name of 1–256 UTF-8 bytes")?;
    validate_message_data(data).map_err(|error| error.to_string())
}
