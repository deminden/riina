//! Portable state transitions for plugin-owned sidebar webviews.

use crate::plugin::{validate_message_data, validate_message_name, validate_plugin_relative_path};
use serde_json::Value;
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum PluginSidebarEffect {
    LoadFile(PathBuf),
    Show,
    Hide,
    PostMessage { name: String, data: Value },
    DeliverMessage { name: String, data: Value },
    ListenMessage(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginSidebarIntent {
    LoadFile(PathBuf),
    Show,
    Hide,
    PostMessage { name: String, data: Value },
    OnMessage(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PluginSidebarState {
    loaded_file: Option<PathBuf>,
    visible: bool,
    message_listeners: BTreeSet<String>,
}

impl PluginSidebarState {
    pub fn loaded_file(&self) -> Option<&PathBuf> {
        self.loaded_file.as_ref()
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn apply(
        &mut self,
        intent: PluginSidebarIntent,
    ) -> Result<Option<PluginSidebarEffect>, String> {
        let effect = match intent {
            PluginSidebarIntent::LoadFile(path) => {
                validate_plugin_relative_path(&path)?;
                self.loaded_file = Some(path.clone());
                self.message_listeners.clear();
                Some(PluginSidebarEffect::LoadFile(path))
            }
            PluginSidebarIntent::Show if self.loaded_file.is_some() => {
                self.visible = true;
                Some(PluginSidebarEffect::Show)
            }
            PluginSidebarIntent::Hide if self.loaded_file.is_some() => {
                self.visible = false;
                Some(PluginSidebarEffect::Hide)
            }
            PluginSidebarIntent::PostMessage { name, data } if self.loaded_file.is_some() => {
                validate_message_name(&name)?;
                validate_message_data(&data)?;
                Some(PluginSidebarEffect::PostMessage { name, data })
            }
            PluginSidebarIntent::OnMessage(name) if self.loaded_file.is_some() => {
                validate_message_name(&name)?;
                if self.message_listeners.insert(name.clone()) {
                    Some(PluginSidebarEffect::ListenMessage(name))
                } else {
                    None
                }
            }
            PluginSidebarIntent::Show
            | PluginSidebarIntent::Hide
            | PluginSidebarIntent::PostMessage { .. }
            | PluginSidebarIntent::OnMessage(_) => None,
        };
        Ok(effect)
    }

    pub fn receive_message(
        &self,
        name: String,
        data: Value,
    ) -> Result<Option<PluginSidebarEffect>, String> {
        validate_message_name(&name)?;
        validate_message_data(&data)?;
        Ok(self
            .message_listeners
            .contains(&name)
            .then_some(PluginSidebarEffect::DeliverMessage { name, data }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_loads_only_confined_files_and_resets_message_listeners() {
        let mut state = PluginSidebarState::default();
        for path in ["", "../outside.html", "assets/../../outside.html"] {
            assert!(
                state
                    .apply(PluginSidebarIntent::LoadFile(path.into()))
                    .is_err()
            );
        }
        assert_eq!(state.loaded_file(), None);
        assert_eq!(
            state
                .apply(PluginSidebarIntent::LoadFile("sidebar/index.html".into()))
                .unwrap(),
            Some(PluginSidebarEffect::LoadFile("sidebar/index.html".into()))
        );
        assert_eq!(
            state.apply(PluginSidebarIntent::OnMessage("ready".into())),
            Ok(Some(PluginSidebarEffect::ListenMessage("ready".into())))
        );
        assert_eq!(
            state.apply(PluginSidebarIntent::LoadFile("sidebar/next.html".into())),
            Ok(Some(PluginSidebarEffect::LoadFile(
                "sidebar/next.html".into()
            )))
        );
        assert_eq!(
            state.receive_message("ready".into(), Value::Null).unwrap(),
            None
        );
    }

    #[test]
    fn visibility_and_messages_require_a_loaded_sidebar_document() {
        let mut state = PluginSidebarState::default();
        assert_eq!(state.apply(PluginSidebarIntent::Show), Ok(None));
        assert!(!state.is_visible());
        assert_eq!(state.apply(PluginSidebarIntent::Hide), Ok(None));
        assert_eq!(
            state.apply(PluginSidebarIntent::OnMessage("ready".into())),
            Ok(None)
        );
        assert_eq!(
            state.apply(PluginSidebarIntent::PostMessage {
                name: "ready".into(),
                data: Value::Null,
            }),
            Ok(None)
        );

        state
            .apply(PluginSidebarIntent::LoadFile("sidebar/index.html".into()))
            .unwrap();
        assert_eq!(
            state.apply(PluginSidebarIntent::Show),
            Ok(Some(PluginSidebarEffect::Show))
        );
        assert!(state.is_visible());
        assert_eq!(
            state.apply(PluginSidebarIntent::OnMessage("ready".into())),
            Ok(Some(PluginSidebarEffect::ListenMessage("ready".into())))
        );
        assert_eq!(
            state.receive_message("ready".into(), serde_json::json!({"ok": true})),
            Ok(Some(PluginSidebarEffect::DeliverMessage {
                name: "ready".into(),
                data: serde_json::json!({"ok": true}),
            }))
        );
        assert_eq!(
            state.apply(PluginSidebarIntent::PostMessage {
                name: "reply".into(),
                data: serde_json::json!({"ok": true}),
            }),
            Ok(Some(PluginSidebarEffect::PostMessage {
                name: "reply".into(),
                data: serde_json::json!({"ok": true}),
            }))
        );
        assert_eq!(
            state.apply(PluginSidebarIntent::Hide),
            Ok(Some(PluginSidebarEffect::Hide))
        );
        assert!(!state.is_visible());
    }
}
