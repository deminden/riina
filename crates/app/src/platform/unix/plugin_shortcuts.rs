//! Let the owning player handle its fixed menu shortcuts while GTK owns focus.

use crate::{PlayerWindow, plugin_runtime::standalone::Endpoint};
use gtk::{gdk::ModifierType, glib::Propagation, prelude::WidgetExt};
use riina_ui::{PlayerCropState, PlayerDialogState, PlayerMenuActions, PlayerPreferences};
use slint::ComponentHandle;
use std::sync::Weak;
use wry::{WebView, WebViewExtUnix};

const CONTROL: ModifierType = ModifierType::CONTROL_MASK;
const CONTROL_SHIFT: ModifierType = CONTROL.union(ModifierType::SHIFT_MASK);
const CONTROL_ALT: ModifierType = CONTROL.union(ModifierType::MOD1_MASK);
const CONTROL_ALT_SHIFT: ModifierType = CONTROL_ALT.union(ModifierType::SHIFT_MASK);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuShortcut {
    Preferences,
    Hide,
    Quit,
    OpenFile,
    OpenFileAlternative,
    OpenUrl,
    OpenUrlAlternative,
    NewWindow,
    History,
    Inspector,
    LogViewer,
    Close,
    Jump,
    VideoFilters,
    AudioFilters,
    Minimize,
    Help,
}

impl MenuShortcut {
    fn from_key(key: char, modifiers: ModifierType) -> Option<Self> {
        // These fixed accelerators are also declared in player-menu-window.slint.
        // The WebView keeps text editing keys and ordinary playback bindings.
        match (key.to_ascii_lowercase(), modifiers) {
            (',', CONTROL) => Some(Self::Preferences),
            ('h', CONTROL) => Some(Self::Hide),
            ('q', CONTROL) => Some(Self::Quit),
            ('o', CONTROL) => Some(Self::OpenFile),
            ('o', CONTROL_ALT) => Some(Self::OpenFileAlternative),
            ('o', CONTROL_SHIFT) => Some(Self::OpenUrl),
            ('o', CONTROL_ALT_SHIFT) => Some(Self::OpenUrlAlternative),
            ('n', CONTROL) => Some(Self::NewWindow),
            ('h', CONTROL_SHIFT) => Some(Self::History),
            ('i', CONTROL) => Some(Self::Inspector),
            ('l', CONTROL_ALT) => Some(Self::LogViewer),
            ('w', CONTROL) => Some(Self::Close),
            ('j', CONTROL) => Some(Self::Jump),
            ('f', CONTROL_SHIFT) => Some(Self::VideoFilters),
            ('g', CONTROL_SHIFT) => Some(Self::AudioFilters),
            ('m', CONTROL) => Some(Self::Minimize),
            ('/' | '?', CONTROL_SHIFT) => Some(Self::Help),
            _ => None,
        }
    }

    fn enabled(self, app: &PlayerWindow) -> bool {
        if app.global::<PlayerDialogState>().get_dialog_open() {
            return false;
        }
        let editing = app.global::<PlayerCropState>().get_crop_editing();
        match self {
            Self::Preferences
            | Self::OpenFile
            | Self::OpenFileAlternative
            | Self::OpenUrl
            | Self::OpenUrlAlternative => !editing,
            Self::NewWindow => {
                !editing
                    && app
                        .global::<PlayerPreferences>()
                        .get_enable_new_window_shortcut()
            }
            Self::Jump | Self::VideoFilters | Self::AudioFilters => app.get_loaded() && !editing,
            _ => true,
        }
    }

    fn invoke(self, app: &PlayerWindow) {
        let actions = app.global::<PlayerMenuActions>();
        match self {
            Self::Preferences => actions.invoke_open_preferences(),
            Self::Hide => actions.invoke_hide_riina(),
            Self::Quit => actions.invoke_quit_riina(),
            Self::OpenFile => actions.invoke_open_file(),
            Self::OpenFileAlternative => actions.invoke_open_file_alternative(),
            Self::OpenUrl => actions.invoke_open_url_dialog(),
            Self::OpenUrlAlternative => actions.invoke_open_url_alternative(),
            Self::NewWindow => actions.invoke_create_welcome_window(),
            Self::History => actions.invoke_show_playback_history(),
            Self::Inspector => actions.invoke_show_inspector(),
            Self::LogViewer => actions.invoke_show_log_viewer(),
            Self::Close => actions.invoke_close_player(),
            Self::Jump => actions.invoke_open_jump_time_dialog(),
            Self::VideoFilters => actions.invoke_open_video_filters(),
            Self::AudioFilters => actions.invoke_open_audio_filters(),
            Self::Minimize => actions.invoke_minimize_window(),
            Self::Help => actions.invoke_open_help_link("help".into()),
        }
    }
}

pub(crate) fn bind(
    webview: &WebView,
    owner: slint::Weak<PlayerWindow>,
    standalone: Option<Weak<Endpoint>>,
) {
    let accelerator_modifiers = gtk::accelerator_get_default_mod_mask();
    webview.webview().connect_key_press_event(move |_, event| {
        let modifiers = event.state() & accelerator_modifiers;
        let Some(shortcut) = event
            .keyval()
            .to_unicode()
            .and_then(|key| MenuShortcut::from_key(key, modifiers))
        else {
            return Propagation::Proceed;
        };
        if shortcut == MenuShortcut::Close
            && let Some(standalone) = &standalone
        {
            // The presenter hides the WebView after the native key event.
            if let Some(window) = standalone.upgrade() {
                window.user_closed();
            }
            return Propagation::Stop;
        }
        if !owner.upgrade().is_some_and(|app| shortcut.enabled(&app)) {
            return Propagation::Proceed;
        }
        // A callback can unmap the child or close the entire player. Invoke it
        // after the native key handler has finished.
        let queued = crate::background::to_ui(owner.clone(), move |app| {
            if shortcut.enabled(&app) {
                shortcut.invoke(&app);
            }
        });
        if queued.is_ok() {
            Propagation::Stop
        } else {
            Propagation::Proceed
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_keeps_text_editing_and_distinguishes_menu_modifiers() {
        for edit in ['a', 'c', 'v', 'x', 'z'] {
            assert_eq!(MenuShortcut::from_key(edit, CONTROL), None);
        }
        for letter in ['h', 'q', 'o', 'w', 'm', ' '] {
            assert_eq!(MenuShortcut::from_key(letter, ModifierType::empty()), None);
        }
        assert_eq!(
            MenuShortcut::from_key('H', CONTROL),
            Some(MenuShortcut::Hide)
        );
        assert_eq!(
            MenuShortcut::from_key('H', CONTROL_SHIFT),
            Some(MenuShortcut::History)
        );
        assert_eq!(MenuShortcut::from_key('q', CONTROL_SHIFT), None);
        assert_eq!(
            MenuShortcut::from_key('O', CONTROL_ALT),
            Some(MenuShortcut::OpenFileAlternative)
        );
        assert_eq!(
            MenuShortcut::from_key('o', CONTROL_SHIFT),
            Some(MenuShortcut::OpenUrl)
        );
        assert_eq!(
            MenuShortcut::from_key('o', CONTROL_ALT.union(ModifierType::SHIFT_MASK)),
            Some(MenuShortcut::OpenUrlAlternative)
        );
        assert_eq!(
            MenuShortcut::from_key('n', CONTROL),
            Some(MenuShortcut::NewWindow)
        );
    }
}
