//! Present the last active playback chain alongside persistent presets.
use super::profile::Edit;
use super::{
    ApplicationHistory, Kind, PlayerWindow, Profile, Target, preset_editor, saved_filter_rows,
};
use crate::{SavedFilterMenuRow, VideoFilterWindowRow, VideoFiltersWindow};
use riina_core::{
    Command, Phase,
    crop::VideoFilter,
    sync::lock,
    video_filters::{self, SavedFilter},
};
use slint::{
    CloseRequestResponse, ComponentHandle, Model, ModelRc, Timer, VecModel,
    winit_030::WinitWindowAccessor,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};

pub(super) struct Slot {
    window: RefCell<Option<(VideoFiltersWindow, Rc<Editor>)>>,
    refresh: Rc<Timer>,
    closed: Weak<crate::windows::close_observer::CloseObserver>,
}

impl Slot {
    pub(super) fn new(closed: &Rc<crate::windows::close_observer::CloseObserver>) -> Self {
        Self {
            window: RefCell::default(),
            refresh: Rc::new(Timer::default()),
            closed: Rc::downgrade(closed),
        }
    }

    pub(super) fn show(
        &self,
        profile: &Rc<Profile>,
        history: &Rc<ApplicationHistory>,
        kind: Kind,
        anchor: Option<&PlayerWindow>,
    ) -> Result<(), String> {
        let mut slot = self.window.borrow_mut();
        let (window, editor) = match slot.as_ref() {
            Some((window, editor)) => (window.clone_strong(), editor.clone()),
            None => {
                let window = VideoFiltersWindow::new().map_err(|error| error.to_string())?;
                crate::appearance_preferences::install_system_typography(&window);
                window.set_is_audio(matches!(kind, Kind::Audio));
                let presets = match kind {
                    Kind::Video => &video_filters::PRESET_LABELS[..],
                    Kind::Audio => &video_filters::AUDIO_PRESET_LABELS[..],
                };
                window.set_presets(ModelRc::new(VecModel::from(
                    std::iter::once(window.get_preset_prompt())
                        .chain(presets.iter().copied().map(slint::SharedString::from))
                        .collect::<Vec<_>>(),
                )));
                if let Some(anchor) = anchor {
                    crate::platform::center_auxiliary_window(&window, anchor);
                }
                let editor = Rc::new(Editor {
                    profile: Rc::downgrade(profile),
                    history: Rc::downgrade(history),
                    kind,
                    view: RefCell::default(),
                    saved: RefCell::default(),
                    selected: RefCell::default(),
                });
                preset_editor::install(&window, matches!(kind, Kind::Audio));
                editor.bind(&window);
                let refresh = Rc::downgrade(&self.refresh);
                let closed = self.closed.clone();
                window.window().on_close_requested(move || {
                    if let Some(refresh) = refresh.upgrade() {
                        refresh.stop();
                    }
                    if let Some(closed) = closed.upgrade() {
                        closed.notify();
                    }
                    CloseRequestResponse::HideWindow
                });
                *slot = Some((window.clone_strong(), editor.clone()));
                (window, editor)
            }
        };
        drop(slot);
        editor.refresh(&window, profile);
        crate::platform::show_window(&window).map_err(|error| error.to_string())?;
        crate::platform::enable_video_filters_acrylic(&window);
        window
            .window()
            .with_winit_window(|host| host.focus_window());
        if !self.refresh.running() {
            super::super::visible_refresh::run(
                &window,
                &self.refresh,
                Duration::from_millis(300),
                move |window| {
                    if let Some(profile) = editor.profile.upgrade() {
                        editor.refresh(window, &profile);
                    }
                },
            );
        }
        Ok(())
    }

    pub(super) fn is_visible(&self) -> bool {
        self.window
            .borrow()
            .as_ref()
            .is_some_and(|(window, _)| window.window().is_visible())
    }

    pub(super) fn hide(&self) {
        self.refresh.stop();
        if let Some((window, _)) = self.window.borrow().as_ref() {
            let _ = window.hide();
        }
    }
}

struct LiveView {
    target: Weak<Target>,
    generation: u64,
    filters: Vec<VideoFilter>,
}

impl LiveView {
    fn same_media(&self, other: &Self) -> bool {
        self.target.ptr_eq(&other.target) && self.generation == other.generation
    }

    fn matches(&self, other: &Self) -> bool {
        self.same_media(other) && self.filters == other.filters
    }
}

#[derive(Default)]
struct SavedRows {
    filters: Vec<SavedFilter>,
    rows: Vec<SavedFilterMenuRow>,
}

struct Editor {
    profile: Weak<Profile>,
    history: Weak<ApplicationHistory>,
    kind: Kind,
    view: RefCell<Option<LiveView>>,
    saved: RefCell<SavedRows>,
    selected: RefCell<Option<SavedFilter>>,
}

macro_rules! bind_action {
    (
        $editor:expr, $window:expr, $callback:ident,
        |$state:ident, $profile:ident, $view:ident $(, $arg:ident)+| $body:block
    ) => {{
        let editor = Rc::downgrade($editor);
        let window = $window.as_weak();
        $window.$callback(move |$($arg),+| {
            if let (Some(editor), Some(window)) = (editor.upgrade(), window.upgrade()) {
                editor.action(&window, |$state, $profile, $view| $body);
            }
        });
    }};
}

impl Editor {
    fn active(&self, profile: &Profile) -> Option<Rc<Target>> {
        let history = self.history.upgrade()?;
        profile.last_active(&history)
    }

    fn sample(&self, target: &Rc<Target>) -> Option<LiveView> {
        let playback = lock(&target.playback);
        if matches!(
            playback.phase,
            Phase::Stopped | Phase::Loading | Phase::Failed
        ) {
            return None;
        }
        Some(LiveView {
            target: Rc::downgrade(target),
            generation: playback.generation,
            filters: self.kind.filters(&playback)?.to_vec(),
        })
    }

    fn refresh(&self, window: &VideoFiltersWindow, profile: &Profile) {
        let target = self.active(profile);
        let (theme, fallback_dark) = profile.theme();
        let dark = target
            .as_ref()
            .and_then(|target| target.app.upgrade())
            .map_or_else(
                || {
                    let system_dark = window
                        .window()
                        .with_winit_window(|host| host.theme())
                        .flatten()
                        .map_or(fallback_dark, |theme| {
                            theme == slint::winit_030::winit::window::Theme::Dark
                        });
                    theme.dark_for(system_dark)
                },
                |app| app.global::<riina_ui::PlayerAppearance>().get_dark_theme(),
            );
        window.set_dark_theme(dark);
        let view = target.as_ref().and_then(|target| self.sample(target));
        let active = view
            .as_ref()
            .map_or(&[][..], |view| view.filters.as_slice());
        let saved = profile.saved(self.kind);
        let rows = saved_filter_rows(&saved, active);
        if self.saved.borrow().rows != rows {
            window.set_saved_filters(ModelRc::new(VecModel::from(rows.clone())));
        }
        self.saved.replace(SavedRows {
            filters: saved,
            rows,
        });
        window.set_live_filter_controls(view.is_some());

        let mut current = self.view.borrow_mut();
        let changed = match (current.as_ref(), view.as_ref()) {
            (Some(previous), Some(next)) => !previous.matches(next),
            (None, None) => false,
            _ => true,
        };
        if changed {
            reset_current_edit(window);
            window.set_current_filters(ModelRc::new(VecModel::from(
                active
                    .iter()
                    .enumerate()
                    .map(|(index, filter)| {
                        let formatted =
                            slint::SharedString::from(video_filters::format_filter(filter));
                        VideoFilterWindowRow {
                            token: index.to_string().into(),
                            name: filter.name.clone().into(),
                            detail: formatted.clone(),
                            filter: formatted,
                        }
                    })
                    .collect::<Vec<_>>(),
            )));
        }
        *current = view;
    }

    fn send(
        &self,
        profile: &Profile,
        chain_sensitive: bool,
        feedback_name: Option<String>,
        command: impl FnOnce(&LiveView) -> Result<Command, String>,
    ) -> Result<(), String> {
        let (target, live) = self
            .active(profile)
            .and_then(|target| Some((target.clone(), self.sample(&target)?)))
            .ok_or("Open media to change active filters")?;
        let presented = self.view.borrow();
        let expected = presented
            .as_ref()
            .filter(|expected| expected.same_media(&live))
            .ok_or("Playback changed. Try again with the current player.")?;
        if chain_sensitive && expected.filters != live.filters {
            return Err("The active filters changed. Select the filter again.".into());
        }
        let command = command(&live)?;
        match feedback_name {
            Some(name) => target
                .player
                .send_filter_edit(live.generation, command, name),
            None => target.player.send_for_media(live.generation, command),
        }
        .map_err(|error| error.to_string())
    }

    fn action(
        self: &Rc<Self>,
        window: &VideoFiltersWindow,
        action: impl FnOnce(&Self, &Profile, &VideoFiltersWindow) -> Result<(), String>,
    ) {
        if let Some(profile) = self.profile.upgrade() {
            let result = action(self, &profile, window);
            self.refresh(window, &profile);
            window.set_filter_status(result.err().unwrap_or_default().into());
        }
    }

    fn bind(self: &Rc<Self>, window: &VideoFiltersWindow) {
        bind_action!(
            self,
            window,
            on_add_filter,
            |editor, profile, window, raw| {
                let raw = valid_raw(&raw)?;
                let name = usize::try_from(window.get_preset_index())
                    .ok()
                    .and_then(|index| window.get_presets().row_data(index))
                    .filter(|_| window.get_preset_index() > 0)
                    .map_or_else(|| raw.clone(), |label| label.to_string());
                editor.send(profile, false, Some(name), |_| Ok(editor.kind.add(raw)))?;
                window.set_filter_string("".into());
                Ok(())
            }
        );
        bind_action!(
            self,
            window,
            on_remove_current_filter,
            |editor, profile, _window, token| {
                editor.send(profile, true, Some(String::new()), |live| {
                    let filter = token
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| live.filters.get(index))
                        .cloned()
                        .ok_or("The selected filter is no longer active")?;
                    Ok(editor.kind.remove(filter))
                })
            }
        );
        bind_action!(
            self,
            window,
            on_apply_current_filter,
            |editor, profile, window, index, raw| {
                editor.send(profile, true, None, |live| {
                    let index = usize::try_from(index)
                        .ok()
                        .filter(|index| *index < live.filters.len())
                        .ok_or("The selected filter is no longer active")?;
                    Ok(editor.kind.replace(index, valid_raw(&raw)?))
                })?;
                reset_current_edit(window);
                Ok(())
            }
        );
        bind_action!(
            self,
            window,
            on_toggle_saved_filter,
            |editor, profile, window, raw| {
                let raw = valid_raw(&raw)?;
                let name = window.get_editing_saved_name();
                let name = if name.trim().is_empty() {
                    raw.clone()
                } else {
                    name.to_string()
                };
                editor.send(profile, false, Some(name), |_| Ok(editor.kind.toggle(raw)))
            }
        );
        self.bind_presets(window);
    }

    fn saved_at(&self, index: i32) -> Result<SavedFilter, String> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.saved.borrow().filters.get(index).cloned())
            .ok_or_else(|| "The selected saved filter no longer exists".into())
    }

    fn bind_presets(self: &Rc<Self>, window: &VideoFiltersWindow) {
        bind_action!(
            self,
            window,
            on_select_saved_filter,
            |editor, _profile, _window, index| {
                editor.selected.replace(Some(editor.saved_at(index)?));
                Ok(())
            }
        );
        bind_action!(
            self,
            window,
            on_save_current_filter,
            |editor, profile, window, filter, name| {
                profile.edit(
                    editor.kind,
                    Edit::Add(SavedFilter {
                        name: name.to_string(),
                        filter: filter.to_string(),
                        shortcut_key: String::new(),
                        shortcut_modifiers: 0,
                    }),
                )?;
                window.set_filter_name("".into());
                Ok(())
            }
        );
        bind_action!(
            self,
            window,
            on_remove_saved_filter,
            |editor, profile, window, index| {
                let original = editor.saved_at(index)?;
                let selected = editor.selected.borrow().as_ref() == Some(&original);
                profile.edit(editor.kind, Edit::Remove(original))?;
                if selected {
                    editor.selected.replace(None);
                    clear_saved_edit(window);
                }
                Ok(())
            }
        );
        bind_action!(
            self,
            window,
            on_update_saved_filter,
            |editor, profile, window, name, filter, key, modifiers| {
                let original = editor
                    .selected
                    .borrow()
                    .clone()
                    .ok_or("Select a saved filter to edit")?;
                let edited = SavedFilter {
                    name: name.to_string(),
                    filter: filter.to_string(),
                    shortcut_key: key.to_string(),
                    shortcut_modifiers: u8::try_from(modifiers)
                        .map_err(|_| "The saved filter shortcut is invalid")?,
                };
                let saved_name = edited.name.trim().to_owned();
                profile.edit(editor.kind, Edit::Replace { original, edited })?;
                let selected = profile
                    .saved(editor.kind)
                    .into_iter()
                    .find(|row| row.name == saved_name);
                if selected.is_none() {
                    clear_saved_edit(window);
                }
                editor.selected.replace(selected);
                Ok(())
            }
        );
    }
}

fn valid_raw(raw: &str) -> Result<String, String> {
    video_filters::valid_filter_string(raw)
        .then(|| raw.to_owned())
        .ok_or_else(|| "The filter string is empty or invalid".into())
}

fn reset_current_edit(window: &VideoFiltersWindow) {
    window.set_editing_current_index(-1);
    window.set_editing_current_filter("".into());
}

fn clear_saved_edit(window: &VideoFiltersWindow) {
    window.set_editing_saved_index(-1);
    window.set_editing_saved_name("".into());
    window.set_editing_saved_filter("".into());
    window.set_editing_saved_shortcut_key("".into());
    window.set_editing_saved_shortcut_label("".into());
    window.set_editing_saved_shortcut_modifiers(0);
    window.set_shortcut_recording(false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlaybackStore,
        playback_history_window::Archive,
        player::surface::PlayerDisplay,
        video_filter_window::{ApplicationFilters, install},
    };
    use riina_core::{Playback, Preferences};
    use riina_mpv::CommandSender;
    use slint::Model;
    use std::sync::Arc;

    #[test]
    fn same_filter_editor_retargets_playback_and_keeps_presets_without_players()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let directory = riina_test_support::TempDir::new("app-filters-editor")?;
        let path = directory.path().join("preferences.json");
        let owner = ApplicationFilters::default();
        let profile = owner.profile(&path, &Preferences::default());
        let history = Rc::new(ApplicationHistory::default());
        let slot = Slot::new(&owner.closed);
        slot.show(&profile, &history, Kind::Video, None)?;
        let (window, editor) = slot
            .window
            .borrow()
            .as_ref()
            .map(|(window, editor)| (window.clone_strong(), editor.clone()))
            .unwrap();

        assert!(!window.get_live_filter_controls());
        window.invoke_save_current_filter("hflip".into(), "Flip".into());
        assert_eq!(window.get_saved_filters().row_count(), 1);
        assert!(window.get_filter_status().is_empty());
        window.invoke_toggle_saved_filter("hflip".into());
        assert!(!window.get_filter_status().is_empty());

        let register = || -> Result<_, slint::PlatformError> {
            let app = PlayerWindow::new()?;
            let (sender, engine) = CommandSender::recording(4);
            let playback: PlaybackStore = Arc::new(std::sync::Mutex::new(Arc::new(Playback {
                generation: 1,
                phase: Phase::Paused,
                video_filters: Some(vec![VideoFilter {
                    name: "hflip".into(),
                    label: None,
                    enabled: Some(true),
                    params: Default::default(),
                }]),
                ..Playback::default()
            })));
            let archive = Archive {
                history: Arc::default(),
                preferences_path: path.clone(),
                cleared: Arc::default(),
            };
            let route = history.install(
                &app,
                sender.clone(),
                archive,
                Arc::new(|_| Ok(())),
                PlayerDisplay::default(),
            );
            let filters = install(&app, playback.clone(), sender, &profile, &history);
            Ok((app, route, filters, engine, playback))
        };
        let (first, first_route, first_filters, first_engine, first_playback) = register()?;
        let (second, second_route, second_filters, second_engine, _) = register()?;
        first.invoke_history_player_activated();
        editor.refresh(&window, &profile);
        assert!(window.get_live_filter_controls());
        assert_eq!(window.get_current_filters().row_count(), 1);
        window.invoke_toggle_saved_filter("hflip".into());
        assert!(
            matches!(first_engine.commands.try_recv()?, Command::ToggleVideoFilter(raw) if raw == "hflip")
        );

        Arc::make_mut(&mut lock(&first_playback)).generation += 1;
        window.invoke_remove_current_filter("0".into());
        assert!(first_engine.commands.try_recv().is_err());
        assert!(!window.get_filter_status().is_empty());

        second.invoke_history_player_activated();
        editor.refresh(&window, &profile);
        window.invoke_toggle_saved_filter("hflip".into());
        assert!(
            matches!(second_engine.commands.try_recv()?, Command::ToggleVideoFilter(raw) if raw == "hflip")
        );
        drop((first_route, first_filters, second_route, second_filters));
        editor.refresh(&window, &profile);
        assert!(window.window().is_visible());
        assert!(!window.get_live_filter_controls());
        assert_eq!(window.get_current_filters().row_count(), 0);
        assert_eq!(
            window.get_saved_filters().row_data(0).unwrap().label,
            "Flip"
        );
        slot.show(&profile, &history, Kind::Video, None)?;
        let retained = slot.window.borrow();
        assert!(std::ptr::eq(
            retained.as_ref().unwrap().0.window(),
            window.window()
        ));
        window.hide()?;
        Ok(())
    }
}
