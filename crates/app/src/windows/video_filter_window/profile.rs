//! Keep shared presets and commit individual edits against the current profile.
use super::{ApplicationHistory, PlayerWindow, Target, editor::Slot, project_saved_menu};
use riina_core::{
    Command, Playback, Preferences,
    crop::VideoFilter,
    sync::lock,
    video_filters::{SavedFilter, Settings},
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

pub(super) type SharedSettings = Arc<Mutex<Settings>>;

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Video,
    Audio,
}

impl Kind {
    #[cfg(target_os = "macos")]
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Video => "Video Filters",
            Self::Audio => "Audio Filters",
        }
    }

    pub(super) fn filters(self, state: &Playback) -> Option<&[VideoFilter]> {
        match self {
            Self::Video => state.video_filters.as_deref(),
            Self::Audio => state.audio_filters.as_deref(),
        }
    }

    fn preferences(self, profile: &mut Preferences) -> &mut Settings {
        match self {
            Self::Video => &mut profile.video_filters,
            Self::Audio => &mut profile.audio_filters,
        }
    }

    pub(super) fn toggle(self, raw: String) -> Command {
        match self {
            Self::Video => Command::ToggleVideoFilter(raw),
            Self::Audio => Command::ToggleAudioFilter(raw),
        }
    }

    pub(super) fn add(self, raw: String) -> Command {
        match self {
            Self::Video => Command::AddVideoFilter(raw),
            Self::Audio => Command::AddAudioFilter(raw),
        }
    }

    pub(super) fn remove_at(self, index: usize) -> Command {
        match self {
            Self::Video => Command::RemoveVideoFilterAt(index),
            Self::Audio => Command::RemoveAudioFilterAt(index),
        }
    }

    pub(super) fn replace(self, index: usize, filter: String) -> Command {
        match self {
            Self::Video => Command::SetVideoFilterAt { index, filter },
            Self::Audio => Command::SetAudioFilterAt { index, filter },
        }
    }
}

pub(super) enum Edit {
    Add(SavedFilter),
    Replace {
        original: SavedFilter,
        edited: SavedFilter,
    },
    Remove(SavedFilter),
}

impl Edit {
    fn apply(self, settings: &mut Settings) -> Result<(), String> {
        let result = match self {
            Self::Add(filter) => settings.save_with_shortcut(
                &filter.name,
                &filter.filter,
                &filter.shortcut_key,
                filter.shortcut_modifiers,
            ),
            Self::Replace { original, edited } => settings.update_with_shortcut(
                position(settings, &original)?,
                &edited.name,
                &edited.filter,
                &edited.shortcut_key,
                edited.shortcut_modifiers,
            ),
            Self::Remove(original) => settings.remove(position(settings, &original)?).map(|_| ()),
        };
        result.map_err(|error| error.to_string())
    }
}

fn position(settings: &Settings, original: &SavedFilter) -> Result<usize, String> {
    settings
        .saved
        .iter()
        .position(|saved| saved == original)
        .ok_or_else(|| "The selected saved filter changed in another window".into())
}

pub(crate) struct Profile {
    pub(super) path: PathBuf,
    baseline: RefCell<Preferences>,
    video: SharedSettings,
    audio: SharedSettings,
    windows: [Slot; 2],
    targets: RefCell<Vec<Weak<Target>>>,
}

impl Profile {
    pub(super) fn new(
        path: &Path,
        preferences: &Preferences,
        closed: &Rc<crate::windows::close_observer::CloseObserver>,
    ) -> Self {
        Self {
            path: path.to_owned(),
            baseline: RefCell::new(preferences.clone()),
            video: Arc::new(Mutex::new(preferences.video_filters.clone())),
            audio: Arc::new(Mutex::new(preferences.audio_filters.clone())),
            windows: std::array::from_fn(|_| Slot::new(closed)),
            targets: RefCell::default(),
        }
    }

    pub(crate) fn settings(&self, kind: Kind) -> SharedSettings {
        self.shared_settings(kind).clone()
    }

    fn shared_settings(&self, kind: Kind) -> &SharedSettings {
        match kind {
            Kind::Video => &self.video,
            Kind::Audio => &self.audio,
        }
    }

    pub(super) fn saved(&self, kind: Kind) -> Vec<SavedFilter> {
        lock(self.shared_settings(kind)).saved.clone()
    }

    pub(super) fn register(&self, target: &Rc<Target>) {
        let mut targets = self.targets.borrow_mut();
        targets.retain(|target| target.strong_count() != 0);
        targets.push(Rc::downgrade(target));
        self.project(target, Kind::Video);
        self.project(target, Kind::Audio);
    }

    pub(super) fn last_active(&self, history: &ApplicationHistory) -> Option<Rc<Target>> {
        let (app, _) = history.browser_destination()?;
        self.targets
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .find(|target| target.matches(&app))
    }

    pub(crate) fn show(
        self: &Rc<Self>,
        kind: Kind,
        anchor: Option<&PlayerWindow>,
        history: &Rc<ApplicationHistory>,
    ) -> Result<(), String> {
        if let Ok(Some(preferences)) = crate::preference_load::read_valid(&self.path) {
            self.adopt(preferences);
        }
        self.windows[kind as usize].show(self, history, kind, anchor)
    }

    pub(super) fn theme(&self) -> riina_core::ThemeMaterial {
        self.baseline.borrow().theme_material
    }

    pub(super) fn edit(&self, kind: Kind, edit: Edit) -> Result<(), String> {
        let mut committed = None;
        crate::preference_store::update(&self.path, &self.baseline.borrow(), |preferences| {
            edit.apply(kind.preferences(preferences))?;
            committed = Some(preferences.clone());
            Ok(())
        })?;
        if let Some(preferences) = committed {
            self.adopt(preferences);
            crate::platform::browser_activation::refresh_idle_menu();
        }
        Ok(())
    }

    fn adopt(&self, preferences: Preferences) {
        for (kind, saved) in [
            (Kind::Video, &preferences.video_filters),
            (Kind::Audio, &preferences.audio_filters),
        ] {
            let mut current = lock(self.shared_settings(kind));
            if *current != *saved {
                current.clone_from(saved);
                drop(current);
                for target in self.targets.borrow().iter().filter_map(Weak::upgrade) {
                    self.project(&target, kind);
                }
            }
        }
        self.baseline.replace(preferences);
    }

    fn project(&self, target: &Target, kind: Kind) {
        if let Some(app) = target.app.upgrade() {
            let playback = lock(&target.playback).clone();
            project_saved_menu(
                &app,
                kind,
                &lock(self.shared_settings(kind)).saved,
                kind.filters(&playback).unwrap_or_default(),
            );
        }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.windows.iter().any(Slot::is_visible)
    }

    pub(super) fn hide(&self) {
        for window in &self.windows {
            window.hide();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(name: &str, filter: &str) -> SavedFilter {
        SavedFilter {
            name: name.into(),
            filter: filter.into(),
            shortcut_key: String::new(),
            shortcut_modifiers: 0,
        }
    }

    #[test]
    fn saved_edits_merge_across_profiles_and_stale_selections_cannot_overwrite_a_change()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = riina_test_support::TempDir::new("app-filters-profile-merge")?;
        let path = directory.path().join("preferences.json");
        let fallback = Preferences::default();
        let close = Rc::default();
        let first = Profile::new(&path, &fallback, &close);
        let second = Profile::new(&path, &fallback, &close);
        let vertical = saved("Vertical", "vflip");
        let horizontal = saved("Horizontal", "hflip");
        first.edit(Kind::Video, Edit::Add(vertical.clone()))?;
        second.edit(Kind::Video, Edit::Add(horizontal.clone()))?;
        first.edit(Kind::Audio, Edit::Add(saved("Quiet", "volume=0.5")))?;
        assert_eq!(
            first.saved(Kind::Video),
            [vertical.clone(), horizontal.clone()]
        );

        first.edit(
            Kind::Video,
            Edit::Replace {
                original: vertical.clone(),
                edited: saved("Flip Vertically", "vflip"),
            },
        )?;
        let old = second.saved(Kind::Video);
        assert!(second.edit(Kind::Video, Edit::Remove(vertical)).is_err());
        assert_eq!(second.saved(Kind::Video), old);
        second.edit(Kind::Video, Edit::Remove(horizontal))?;
        assert_eq!(
            second.saved(Kind::Video),
            [saved("Flip Vertically", "vflip")]
        );
        assert_eq!(second.saved(Kind::Audio), [saved("Quiet", "volume=0.5")]);
        Ok(())
    }
}
