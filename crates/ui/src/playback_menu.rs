//! Read the player menu catalog without creating a visible player or decoder.
use crate::{
    CatalogRow, MenuShortcutRow, PlaybackSubtitleState, PlaybackVideoState, PlayerMenuWindow,
    SavedFilterMenuRow, menu_tree::capture_bar,
};
use slint_runtime::{
    ComponentHandle, Keys, ModelRc, PlatformError, SharedString,
    private_unstable_api::re_exports::{Menu, MenuEntry, MenuFromItemTree, SharedVector},
};

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
    pub shortcut: Keys,
    pub children: Vec<Self>,
}

impl PlaybackMenuCatalog {
    pub const FAMILIES: [&str; 4] = ["Playback", "Video", "Audio", "Subtitles"];

    pub fn new() -> Result<Self, PlatformError> {
        let (component, menu) = capture_bar(PlayerMenuWindow::new)?;
        component.set_menu_bar_visible(true);
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
                let root = roots.iter().find(|entry| entry.title.as_str() == *name)?;
                Some(self.snapshot_row(root))
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

    fn snapshot_row(&self, entry: &MenuEntry) -> PlaybackMenuRow {
        let mut children = SharedVector::default();
        if entry.has_sub_menu {
            self.menu.sub_menu(Some(entry), &mut children);
        }
        PlaybackMenuRow {
            title: entry.title.to_string(),
            separator: entry.is_separator,
            shortcut: entry.shortcut.clone(),
            children: children
                .iter()
                .map(|child| self.snapshot_row(child))
                .collect(),
        }
    }
}
