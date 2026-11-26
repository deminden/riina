//! Portable state of a realm-owned standalone WebView window.
use crate::plugin::{
    PluginSurfaceError, checked_message_size, validate_message_name, validate_overlay_text,
    validate_plugin_relative_path,
};
use serde::Deserialize;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

const MAX_QUEUED_MESSAGES: usize = 128;
const MAX_QUEUED_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub enum WindowError {
    Path,
    SimpleModeRequired,
    ContentSize,
    Title,
    Frame,
    MessageName,
    MessageData(PluginSurfaceError),
    MessageQueueFull,
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Path => "Expected a safe relative plugin path",
            Self::SimpleModeRequired => "Style and content require standalone simple mode",
            Self::ContentSize => "Standalone style or content exceeds 1 MiB",
            Self::Title => "The standalone window title is invalid",
            Self::Frame => "The standalone window frame is outside the supported range",
            Self::MessageName => "Expected a message name of 1–256 UTF-8 bytes",
            Self::MessageData(error) => return std::fmt::Display::fmt(error, out),
            Self::MessageQueueFull => "The standalone window message queue is full",
        })
    }
}

impl std::error::Error for WindowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MessageData(error) => Some(error),
            _ => None,
        }
    }
}

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
    pub created: bool,
    pub open_revision: u64,
    pub document: Document,
    pub properties: Properties,
    pub frame: Frame,
    frame_revisions: [u64; 4],
}

impl Presentation {
    pub fn frame_changes_since(&self, earlier: &Self) -> Frame {
        let changed = |axis: usize| self.frame_revisions[axis] != earlier.frame_revisions[axis];
        Frame {
            width: self.frame.width.filter(|_| changed(0)),
            height: self.frame.height.filter(|_| changed(1)),
            x: self.frame.x.filter(|_| changed(2)),
            y: self.frame.y.filter(|_| changed(3)),
        }
    }
}

#[derive(Default)]
pub struct State {
    presentation: Presentation,
    messages: Vec<(String, Value)>,
    message_bytes: usize,
}

impl State {
    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    pub fn apply(&mut self, intent: Intent) -> Result<(), WindowError> {
        let revision = self.presentation.revision.wrapping_add(1);
        match intent {
            Intent::Open | Intent::Close => {
                self.presentation.created = true;
                self.presentation.visible = matches!(intent, Intent::Open);
                if self.presentation.visible {
                    self.presentation.open_revision = revision;
                }
            }
            Intent::LoadFile(path) => {
                validate_plugin_relative_path(&path).map_err(|_| WindowError::Path)?;
                self.replace_document(Document::Package(path));
            }
            Intent::SimpleMode => self.replace_document(Document::Simple {
                style: None,
                content: None,
            }),
            Intent::SetStyle(value) => self.update_simple(true, value)?,
            Intent::SetContent(value) => self.update_simple(false, value)?,
            Intent::SetProperties(overrides) if self.presentation.created => {
                self.presentation.properties.merge(overrides)?;
            }
            Intent::SetProperties(_) => return Ok(()),
            Intent::SetFrame(frame) => {
                self.presentation.frame.merge(frame)?;
                // A plugin can restore a size or position after a manual move,
                // including by repeating its last value for that axis.
                for (stamp, value) in self.presentation.frame_revisions.iter_mut().zip([
                    frame.width,
                    frame.height,
                    frame.x,
                    frame.y,
                ]) {
                    if value.is_some() {
                        *stamp = revision;
                    }
                }
                self.presentation.created = true;
            }
            Intent::PostMessage(name, data) => {
                if self.presentation.created {
                    self.queue_message(name, data)?;
                }
                return Ok(());
            }
        }
        self.presentation.revision = revision;
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
        self.presentation.created = true;
        self.presentation.document_id = self.presentation.document_id.wrapping_add(1);
        self.presentation.document = document;
        self.messages.clear();
        self.message_bytes = 0;
    }

    fn update_simple(&mut self, is_style: bool, value: String) -> Result<(), WindowError> {
        let Document::Simple { style, content } = &mut self.presentation.document else {
            return Err(WindowError::SimpleModeRequired);
        };
        validate_overlay_text(&value).map_err(|_| WindowError::ContentSize)?;
        let slot = if is_style { style } else { content };
        *slot = Some(value.into());
        Ok(())
    }

    fn queue_message(&mut self, name: String, data: Value) -> Result<(), WindowError> {
        let bytes = name.len() + message_size(&name, &data)?;
        if self.messages.len() >= MAX_QUEUED_MESSAGES
            || self.message_bytes.saturating_add(bytes) > MAX_QUEUED_BYTES
        {
            return Err(WindowError::MessageQueueFull);
        }
        self.message_bytes += bytes;
        self.messages.push((name, data));
        Ok(())
    }
}

impl Properties {
    fn merge(&mut self, pending: Self) -> Result<(), WindowError> {
        if pending
            .title
            .as_ref()
            .is_some_and(|value| value.len() > 1024 || value.contains('\0'))
        {
            return Err(WindowError::Title);
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
    fn merge(&mut self, pending: Self) -> Result<(), WindowError> {
        if [pending.width, pending.height]
            .into_iter()
            .flatten()
            .any(|value| !(1.0..=8192.0).contains(&value))
            || [pending.x, pending.y]
                .into_iter()
                .flatten()
                .any(|value| !(-100_000.0..=100_000.0).contains(&value))
        {
            return Err(WindowError::Frame);
        }
        self.width = pending.width.or(self.width);
        self.height = pending.height.or(self.height);
        self.x = pending.x.or(self.x);
        self.y = pending.y.or(self.y);
        Ok(())
    }
}

pub fn validate_message(name: &str, data: &Value) -> Result<(), WindowError> {
    message_size(name, data).map(drop)
}

fn message_size(name: &str, data: &Value) -> Result<usize, WindowError> {
    validate_message_name(name).map_err(|_| WindowError::MessageName)?;
    checked_message_size(data).map_err(WindowError::MessageData)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesced_frame_changes_keep_repeated_and_independent_axes() {
        let mut state = State::default();
        let initial = Frame {
            width: Some(400.0),
            height: Some(250.0),
            x: Some(80.0),
            y: Some(90.0),
        };
        state.apply(Intent::SetFrame(initial)).unwrap();
        let prior = state.presentation().clone();
        let repeated = Frame {
            width: initial.width,
            ..Frame::default()
        };
        let move_x = Frame {
            x: Some(-20.0),
            ..Frame::default()
        };
        state.apply(Intent::SetFrame(repeated)).unwrap();
        state.apply(Intent::SetFrame(move_x)).unwrap();
        state.apply(Intent::SetFrame(Frame::default())).unwrap();
        let invalid = Frame {
            height: Some(-1.0),
            ..Frame::default()
        };
        assert!(matches!(
            state.apply(Intent::SetFrame(invalid)),
            Err(WindowError::Frame)
        ));
        assert_eq!(
            state.presentation().frame_changes_since(&prior),
            Frame {
                width: repeated.width,
                x: move_x.x,
                ..Frame::default()
            }
        );
        assert_eq!(
            state.presentation().frame,
            Frame {
                x: move_x.x,
                ..initial
            }
        );
    }
}
