//! Route focused player input to active libmpv script bindings.
use super::PlayerContext;
use crate::{PlayerWindow, key_binding_preferences::recorded_key};
use riina_core::{
    Command, Playback,
    key_bindings::{MpvInputOwner, SCRIPT_KEY_LIMIT, normalize_mpv_key},
    sync::lock,
};
use riina_ui::{
    PlayerControlActions, PlayerCropState, PlayerDialogState, PlaylistPluginPopupState,
};
use slint::{
    ComponentHandle, SharedString,
    winit_030::winit::{
        event::{ElementState, KeyEvent, WindowEvent},
        keyboard::{Key, ModifiersState, NamedKey},
    },
};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

const OPEN_GRACE: Duration = Duration::from_millis(1500);
const ENABLE: &str = "script-binding console/enable";

#[derive(Default)]
pub(super) struct WindowKeys(ModifiersState);

impl WindowKeys {
    pub(super) fn handle(&mut self, app: &slint::Weak<PlayerWindow>, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => self.0 = modifiers.state(),
            WindowEvent::Focused(false) => self.0 = ModifiersState::empty(),
            WindowEvent::KeyboardInput { event, .. } => {
                return app
                    .upgrade()
                    .is_some_and(|app| route_window_key(&app, event, self.0));
            }
            _ => {}
        }
        false
    }
}

pub(crate) fn route_window_key(
    app: &PlayerWindow,
    event: &KeyEvent,
    modifiers: ModifiersState,
) -> bool {
    if event.logical_key == Key::Named(NamedKey::F10)
        || app.global::<PlayerCropState>().get_crop_editing()
        || app.global::<PlayerDialogState>().get_dialog_open()
        || app.global::<PlaylistPluginPopupState>().get_shown()
    {
        return false;
    }
    let text = match &event.logical_key {
        Key::Character(text) => text.as_str().into(),
        Key::Named(
            name @ (NamedKey::Space
            | NamedKey::Enter
            | NamedKey::Escape
            | NamedKey::Tab
            | NamedKey::Backspace
            | NamedKey::Delete
            | NamedKey::Insert
            | NamedKey::Home
            | NamedKey::End
            | NamedKey::PageUp
            | NamedKey::PageDown
            | NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::ArrowUp
            | NamedKey::ArrowDown),
        ) => format!("{name:?}").into(),
        _ => SharedString::default(),
    };
    app.global::<PlayerControlActions>()
        .invoke_forward_mpv_script_key(
            text,
            modifiers.control_key(),
            modifiers.alt_key(),
            modifiers.shift_key(),
            modifiers.super_key(),
            event.state == ElementState::Released,
        )
}

#[derive(Default)]
struct Routing {
    waiting_until: Option<Instant>,
    early_keys: Vec<String>,
}

impl Routing {
    fn waiting(&mut self, now: Instant) -> bool {
        if self.waiting_until.is_some_and(|until| now < until) {
            return true;
        }
        self.waiting_until = None;
        self.early_keys.clear();
        false
    }

    fn queue(&mut self, keys: Vec<String>) {
        let remaining = SCRIPT_KEY_LIMIT - self.early_keys.len();
        self.early_keys.extend(keys.into_iter().take(remaining));
    }

    fn confirm(&mut self) -> Vec<String> {
        self.waiting_until = None;
        std::mem::take(&mut self.early_keys)
    }
}

pub(super) fn install(app: &PlayerWindow, context: &PlayerContext) {
    let state = Rc::new(RefCell::new(Routing::default()));
    let controls = app.global::<PlayerControlActions>();
    let opening = state.clone();
    let open_context = context.clone();
    controls.on_open_mpv_console(move || {
        let (available, open) = {
            let playback = lock(open_context.playback());
            (playback.mpv_console_available, playback.mpv_console_open)
        };
        let now = Instant::now();
        if open || opening.borrow_mut().waiting(now) {
            return;
        }
        if !available {
            open_context
                .report("The loaded playback engine does not provide an mpv console".to_owned());
            return;
        }
        match open_context
            .commands()
            .send(Command::MpvInputAction(ENABLE.into()))
        {
            Ok(()) => opening.borrow_mut().waiting_until = Some(now + OPEN_GRACE),
            Err(error) => open_context.report(error),
        }
    });

    let confirmed = state.clone();
    let confirm_context = context.clone();
    controls.on_mpv_console_keys_ready(move || {
        send_keys(
            &confirm_context,
            MpvInputOwner::Console,
            confirmed.borrow_mut().confirm(),
        );
    });

    let context = context.clone();
    controls.on_forward_mpv_script_key(move |text, control, alt, shift, meta, released| {
        let mut state = state.borrow_mut();
        if text.as_str() == "F10"
            || text
                .as_str()
                .chars()
                .eq(std::iter::once(char::from(slint::platform::Key::F10)))
        {
            return false;
        }
        let playback = lock(context.playback());
        if !playback.mpv_console_open && !state.waiting(Instant::now()) {
            let key = if playback.mpv_statistics_keys.is_empty() {
                None
            } else {
                recorded_key(text.as_str(), control, alt, shift, meta)
                    .map(|key| normalize_mpv_key(&key))
                    .filter(|key| playback.mpv_statistics_keys.contains(key))
            };
            drop(playback);
            let Some(key) = key else {
                return false;
            };
            if !released {
                send_keys(&context, MpvInputOwner::Statistics, vec![key]);
            }
            return true;
        }
        let ready = playback.mpv_console_open && playback.mpv_console_keys_active;
        drop(playback);
        if !released {
            let keys = native_keys(text.as_str(), control, alt, shift, meta);
            if ready {
                send_keys(&context, MpvInputOwner::Console, state.confirm());
                send_keys(&context, MpvInputOwner::Console, keys);
            } else {
                state.queue(keys);
            }
        }
        true
    });
}

pub(super) fn observe(app: &PlayerWindow, playback: &Playback) {
    let controls = app.global::<PlayerControlActions>();
    let ready = playback.mpv_console_open && playback.mpv_console_keys_active;
    if controls.get_mpv_console_ready() != ready {
        controls.set_mpv_console_ready(ready);
        if ready {
            controls.invoke_mpv_console_keys_ready();
        }
    }
}

fn send_keys(context: &PlayerContext, owner: MpvInputOwner, keys: Vec<String>) {
    if !keys.is_empty() {
        context.send(Command::MpvScriptKeys { owner, keys });
    }
}

fn native_keys(text: &str, control: bool, alt: bool, shift: bool, meta: bool) -> Vec<String> {
    let Some(key) = recorded_key(text, control, alt, shift, meta) else {
        return Vec::new();
    };
    if !control
        && !alt
        && !meta
        && !text.chars().any(char::is_control)
        && (recorded_key(text, false, false, false, false).as_deref() == Some(&text.to_lowercase())
            || matches!(text, " " | "#"))
    {
        // For text, preserve the produced characters, including Shift and Caps Lock.
        // For navigation, preserve the actual Shift modifier for selection.
        return text
            .chars()
            .take(SCRIPT_KEY_LIMIT)
            .map(|ch| ch.to_string())
            .collect();
    }
    vec![key]
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::platform::Key;

    #[test]
    fn console_keys_preserve_text_and_navigation_modifiers() {
        for (text, shift, expected) in [
            ("H", true, vec!["H"]),
            ("É", false, vec!["É"]),
            ("你好", false, vec!["你", "好"]),
            ("#", true, vec!["#"]),
            ("+", true, vec!["+"]),
            (" ", false, vec![" "]),
        ] {
            assert_eq!(native_keys(text, false, false, shift, false), expected);
        }
        let left = char::from(Key::LeftArrow).to_string();
        assert_eq!(
            native_keys(&left, false, false, true, false),
            ["Shift+LEFT"]
        );
        assert_eq!(native_keys("v", true, false, false, false), ["Ctrl+v"]);
        assert_eq!(native_keys("Backspace", false, false, false, false), ["BS"]);
        assert!(native_keys("Shift", false, false, false, false).is_empty());
    }

    #[test]
    fn startup_input_is_bounded_drained_in_order_and_discarded_on_timeout() {
        let now = Instant::now();
        let mut state = Routing {
            waiting_until: Some(now + OPEN_GRACE),
            ..Routing::default()
        };
        state.queue(vec!["s".into(), "e".into(), "t".into()]);
        state.queue(vec!["x".into(); SCRIPT_KEY_LIMIT]);
        assert!(state.waiting(now));
        let drained = state.confirm();
        assert_eq!(&drained[..3], ["s", "e", "t"]);
        assert_eq!(drained.len(), SCRIPT_KEY_LIMIT);
        assert!(!state.waiting(now));
        state.waiting_until = Some(now + OPEN_GRACE);
        state.queue(vec!["?".into()]);
        assert!(!state.waiting(now + OPEN_GRACE));
        assert!(state.confirm().is_empty());
    }
}
