//! Own the filter windows independently of playback sessions.
mod editor;
mod preset_editor;
mod profile;

use crate::{PlaybackStore, PlayerWindow, SavedFilterMenuRow};
use riina_core::{Preferences, crop::VideoFilter, sync::lock, video_filters};
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, path::Path, rc::Rc};

use crate::playback_history_window::ApplicationHistory;
pub(crate) use profile::{Kind, Profile};

#[derive(Default)]
pub(crate) struct ApplicationFilters {
    profiles: RefCell<Vec<Rc<Profile>>>,
    pub(crate) closed: Rc<super::close_observer::CloseObserver>,
}

impl ApplicationFilters {
    pub(crate) fn profile(&self, path: &Path, preferences: &Preferences) -> Rc<Profile> {
        let mut profiles = self.profiles.borrow_mut();
        if let Some(profile) = profiles.iter().find(|profile| profile.path == path) {
            return profile.clone();
        }
        let profile = Rc::new(Profile::new(path, preferences, &self.closed));
        profiles.push(profile.clone());
        profile
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.profiles
            .borrow()
            .iter()
            .any(|profile| profile.is_visible())
    }

    pub(crate) fn hide(&self) {
        for profile in self.profiles.borrow().iter() {
            profile.hide();
        }
    }
}

pub(crate) struct Registration {
    // The app and its tool windows refer to this session only while it is registered.
    _target: Rc<Target>,
}

struct Target {
    app: slint::Weak<PlayerWindow>,
    playback: PlaybackStore,
    player: riina_mpv::CommandSender,
}

impl Target {
    fn matches(&self, app: &PlayerWindow) -> bool {
        self.app
            .upgrade()
            .is_some_and(|registered| std::ptr::eq(registered.window(), app.window()))
    }

    fn toggle_filter(
        &self,
        kind: Kind,
        raw: String,
        name: String,
    ) -> Result<(), riina_mpv::EngineError> {
        let generation = lock(&self.playback).generation;
        self.player
            .send_filter_edit(generation, kind.toggle(raw), name)
    }
}

pub(crate) fn install(
    app: &PlayerWindow,
    playback: PlaybackStore,
    player: riina_mpv::CommandSender,
    profile: &Rc<Profile>,
    history: &Rc<ApplicationHistory>,
) -> Registration {
    let target = Rc::new(Target {
        app: app.as_weak(),
        playback,
        player,
    });
    profile.register(&target);
    bind_shortcuts(app, &target, profile);
    bind_menu(app, &target, profile, history, Kind::Video);
    bind_menu(app, &target, profile, history, Kind::Audio);
    Registration { _target: target }
}

fn bind_shortcuts(app: &PlayerWindow, target: &Rc<Target>, profile: &Profile) {
    let shortcut_video_settings = profile.settings(Kind::Video);
    let shortcut_audio_settings = profile.settings(Kind::Audio);
    let target = Rc::downgrade(target);
    app.global::<riina_ui::PlayerControlActions>()
        .on_activate_saved_filter_shortcut(move |key, control, alt, shift, meta| {
            let modifiers = (if control {
                video_filters::SHORTCUT_CONTROL
            } else {
                0
            }) | (if alt { video_filters::SHORTCUT_ALT } else { 0 })
                | (if shift {
                    video_filters::SHORTCUT_SHIFT
                } else {
                    0
                })
                | (if meta {
                    video_filters::SHORTCUT_META
                } else {
                    0
                });
            let matches = |settings: &profile::SharedSettings| {
                lock(settings)
                    .saved
                    .iter()
                    .find(|saved| {
                        saved.shortcut_key == key.as_str() && saved.shortcut_modifiers == modifiers
                    })
                    .cloned()
            };
            let selection = matches(&shortcut_video_settings)
                .map(|saved| (Kind::Video, saved))
                .or_else(|| matches(&shortcut_audio_settings).map(|saved| (Kind::Audio, saved)));
            match (target.upgrade(), selection) {
                (Some(target), Some((kind, saved))) => {
                    target.toggle_filter(kind, saved.filter, saved.name).is_ok()
                }
                _ => false,
            }
        });
}

fn bind_menu(
    app: &PlayerWindow,
    target: &Rc<Target>,
    profile: &Rc<Profile>,
    history: &Rc<ApplicationHistory>,
    kind: Kind,
) {
    let weak = Rc::downgrade(target);
    let toggle = move |filter: slint::SharedString, name: slint::SharedString| {
        if let Some(target) = weak.upgrade()
            && let Some(app) = target.app.upgrade()
            && let Err(error) = target.toggle_filter(kind, filter.to_string(), name.to_string())
        {
            crate::diagnostics::report(&app, error);
        }
    };

    let owner = Rc::downgrade(profile);
    let history = Rc::downgrade(history);
    let weak = target.app.clone();
    let open = move || {
        if let (Some(profile), Some(history), Some(app)) =
            (owner.upgrade(), history.upgrade(), weak.upgrade())
            && let Err(error) = profile.show(kind, Some(&app), &history)
        {
            crate::diagnostics::report(&app, error);
        }
    };
    let menu = app.global::<PlayerMenuActions>();
    match kind {
        Kind::Video => {
            menu.on_toggle_saved_video_filter(toggle);
            menu.on_open_video_filters(open);
        }
        Kind::Audio => {
            menu.on_toggle_saved_audio_filter(toggle);
            menu.on_open_audio_filters(open);
        }
    }
}

fn project_saved_menu(
    app: &PlayerWindow,
    kind: Kind,
    saved: &[video_filters::SavedFilter],
    active: &[VideoFilter],
) {
    let rows = ModelRc::new(VecModel::from(saved_filter_rows(saved, active)));
    match kind {
        Kind::Video => app.set_saved_video_filter_rows(rows),
        Kind::Audio => app.set_saved_audio_filter_rows(rows),
    }
}

pub(crate) fn saved_filter_rows(
    saved_filters: &[video_filters::SavedFilter],
    active_filters: &[riina_core::crop::VideoFilter],
) -> Vec<SavedFilterMenuRow> {
    saved_filters
        .iter()
        .map(|saved| SavedFilterMenuRow {
            token: saved.filter.clone().into(),
            label: saved.name.clone().into(),
            selected: video_filters::is_active(&saved.filter, active_filters),
            shortcut: saved_filter_menu_shortcut(&saved.shortcut_key, saved.shortcut_modifiers),
            shortcut_label: format_shortcut(&saved.shortcut_key, saved.shortcut_modifiers).into(),
            shortcut_key: saved.shortcut_key.clone().into(),
            shortcut_modifiers: i32::from(saved.shortcut_modifiers),
        })
        .collect()
}

fn saved_filter_menu_shortcut(key: &str, modifiers: u8) -> slint::Keys {
    if key.is_empty() {
        return slint::Keys::default();
    }
    let mut parts = Vec::with_capacity(5);
    for (flag, name) in [
        (video_filters::SHORTCUT_META, "Meta"),
        (video_filters::SHORTCUT_CONTROL, "Control"),
        (video_filters::SHORTCUT_ALT, "Alt"),
        (video_filters::SHORTCUT_SHIFT, "Shift"),
    ] {
        if modifiers & flag != 0 {
            parts.push(name.to_owned());
        }
    }
    let mut key = match key.to_ascii_uppercase().as_str() {
        "SPACE" => "Space".to_owned(),
        "TAB" => "Tab".to_owned(),
        "LEFT" | "LEFTARROW" => "LeftArrow".to_owned(),
        "RIGHT" | "RIGHTARROW" => "RightArrow".to_owned(),
        "UP" | "UPARROW" => "UpArrow".to_owned(),
        "DOWN" | "DOWNARROW" => "DownArrow".to_owned(),
        "ENTER" | "RETURN" => "Return".to_owned(),
        "ESC" | "ESCAPE" => "Escape".to_owned(),
        "BS" | "BACKSPACE" => "Backspace".to_owned(),
        "DEL" | "DELETE" => "Delete".to_owned(),
        "-" => "HyphenMinus".to_owned(),
        "=" => "Equals".to_owned(),
        "[" => "OpenBracket".to_owned(),
        "]" => "CloseBracket".to_owned(),
        "\\" => "BackSlash".to_owned(),
        "`" => "BackQuote".to_owned(),
        "," => "Comma".to_owned(),
        "." => "Period".to_owned(),
        "/" => "Slash".to_owned(),
        "<" => "LessThan".to_owned(),
        ">" => "GreaterThan".to_owned(),
        "(" => "OpenParen".to_owned(),
        ")" => "CloseParen".to_owned(),
        "+" => "Plus".to_owned(),
        "_" => "Underscore".to_owned(),
        _ if key.chars().count() == 1 && key.chars().next().is_some_and(char::is_uppercase) => {
            if modifiers & video_filters::SHORTCUT_SHIFT == 0 {
                parts.push("Shift".to_owned());
            }
            key.to_lowercase()
        }
        _ => key.to_owned(),
    };
    parts.push(std::mem::take(&mut key));
    slint::Keys::from_parts(parts.iter().map(String::as_str)).unwrap_or_default()
}

fn format_shortcut(key: &str, modifiers: u8) -> String {
    if key.is_empty() {
        return String::new();
    }
    #[cfg(target_os = "macos")]
    let labels = [
        (video_filters::SHORTCUT_CONTROL, "⌃"),
        (video_filters::SHORTCUT_ALT, "⌥"),
        (video_filters::SHORTCUT_SHIFT, "⇧"),
        (video_filters::SHORTCUT_META, "⌘"),
    ];
    #[cfg(not(target_os = "macos"))]
    let labels = [
        (video_filters::SHORTCUT_CONTROL, "Ctrl+"),
        (video_filters::SHORTCUT_ALT, "Alt+"),
        (video_filters::SHORTCUT_SHIFT, "Shift+"),
        (video_filters::SHORTCUT_META, "Win+"),
    ];
    let mut result = labels
        .iter()
        .filter(|(flag, _)| modifiers & flag != 0)
        .map(|(_, label)| *label)
        .collect::<String>();
    result.push_str(key);
    result
}
