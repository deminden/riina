//! Read the player menu catalog without creating a visible player or decoder.
use crate::{
    CatalogRow, MenuShortcutRow, PlaybackSubtitleState, PlaybackVideoState, PlayerMenuWindow,
    SavedFilterMenuRow, menu_tree::capture_bar,
};
use slint_runtime::{
    ComponentHandle, Keys, ModelRc, PlatformError, SharedString,
    private_unstable_api::re_exports::{Menu, MenuEntry, MenuFromItemTree, SharedVector},
};

const CATALOG_KEY_SEPARATOR: char = '\u{1f}';

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdlePlaybackAction {
    ScreenshotFolder,
    VideoFilters,
    AudioFilters,
}

impl IdlePlaybackAction {
    fn from_catalog_key(key: &str) -> Option<Self> {
        match key {
            "screenshot-folder" => Some(Self::ScreenshotFolder),
            "video-filters" => Some(Self::VideoFilters),
            "audio-filters" => Some(Self::AudioFilters),
            _ => None,
        }
    }
}

/// Catalog keys are added only to the captured copy of the generated menu.
fn catalog_title(raw: &str) -> (&str, &str) {
    raw.strip_prefix(CATALOG_KEY_SEPARATOR)
        .and_then(|entry| entry.split_once(CATALOG_KEY_SEPARATOR))
        .unwrap_or(("", raw))
}

pub struct PlaybackMenuCatalog {
    component: PlayerMenuWindow,
    menu: MenuFromItemTree,
}

#[derive(Default)]
pub struct PlaybackMenuOptions {
    pub aspects: ModelRc<SharedString>,
    pub crops: ModelRc<SharedString>,
    pub subtitle_encodings: ModelRc<CatalogRow>,
    pub video_filters: ModelRc<SavedFilterMenuRow>,
    pub audio_filters: ModelRc<SavedFilterMenuRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackMenuRow {
    pub title: String,
    pub separator: bool,
    pub idle_action: Option<IdlePlaybackAction>,
    pub shortcut: Keys,
    pub children: Vec<Self>,
}

impl PlaybackMenuCatalog {
    const FAMILIES: [&str; 4] = ["playback", "video", "audio", "subtitles"];

    pub fn new() -> Result<Self, PlatformError> {
        let (component, menu) = capture_bar(PlayerMenuWindow::new)?;
        component.set_menu_bar_visible(true);
        component.set_export_playback_menu_keys(true);
        component.set_menu_value_primary_shortcut_is_meta(true);
        component.set_menu_value_live_text_available(true);
        Ok(Self { component, menu })
    }

    pub fn snapshot(&self, shortcuts: ModelRc<MenuShortcutRow>) -> Vec<PlaybackMenuRow> {
        self.component.set_menu_value_menu_shortcuts(shortcuts);
        let mut roots = SharedVector::default();
        self.menu.sub_menu(None, &mut roots);
        Self::FAMILIES
            .iter()
            .filter_map(|name| {
                let root = roots
                    .iter()
                    .find(|entry| catalog_title(&entry.title).0 == *name)?;
                Some(self.snapshot_row(root, 0))
            })
            .collect()
    }

    pub fn set_options(&self, options: PlaybackMenuOptions) {
        let video = self.component.global::<PlaybackVideoState>();
        video.set_video_aspect_menu_labels(options.aspects);
        video.set_video_crop_menu_labels(options.crops);
        self.component
            .global::<PlaybackSubtitleState>()
            .set_subtitle_encoding_options(options.subtitle_encodings);
        self.component
            .set_menu_value_saved_video_filter_rows(options.video_filters);
        self.component
            .set_menu_value_saved_audio_filter_rows(options.audio_filters);
    }

    fn snapshot_row(&self, entry: &MenuEntry, depth: usize) -> PlaybackMenuRow {
        let mut children = SharedVector::default();
        if entry.has_sub_menu {
            self.menu.sub_menu(Some(entry), &mut children);
        }
        // Dynamic names are descendants of the first-level saved-filter and track menus.
        let (key, title) = if depth <= 1 {
            catalog_title(&entry.title)
        } else {
            ("", entry.title.as_str())
        };
        PlaybackMenuRow {
            title: title.to_owned(),
            idle_action: IdlePlaybackAction::from_catalog_key(key),
            separator: entry.is_separator,
            shortcut: entry.shortcut.clone(),
            children: children
                .iter()
                .map(|child| self.snapshot_row(child, depth + 1))
                .collect(),
        }
    }
}
