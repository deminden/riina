//! Collect and restore the General tab's playback and opening policy.
use slint::ComponentHandle;

use super::refresh;
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::{ActionAfterLaunch, DefaultRepeatMode, Preferences};

pub(super) struct Policy {
    pub(super) pause_when_open: bool,
    play_when_entering_fullscreen: bool,
    pause_when_leaving_fullscreen: bool,
    pause_when_inactive: bool,
    pause_when_minimized: bool,
    pause_when_goes_to_sleep: bool,
    quit_when_no_opened_window: bool,
    pub(super) keep_open_on_file_end: bool,
    pub(super) playlist_auto_play_next: bool,
    playlist_auto_add: bool,
    playlist_show_metadata: bool,
    playlist_show_metadata_in_music_mode: bool,
    auto_repeat: bool,
    default_repeat_mode: i32,
    fullscreen_when_open: bool,
    pub(super) resume_last_position: bool,
    pub(super) record_playback_history: bool,
    launch_action: i32,
}

pub(super) struct Draft {
    pub(super) previous: Policy,
    pub(super) selected: Policy,
    repeat_mode: DefaultRepeatMode,
}

impl Draft {
    pub(super) fn read(app: &PlayerWindow, window: &PreferencesWindow) -> Option<Self> {
        let previous = Policy::from_player(app);
        let selected = Policy::from_window(window);
        let Some(repeat_mode) = DefaultRepeatMode::from_index(selected.default_repeat_mode) else {
            app.set_status("Choose a valid default repeat mode".into());
            refresh(window, app);
            return None;
        };
        Some(Self {
            previous,
            selected,
            repeat_mode,
        })
    }

    pub(super) fn put(&self, preferences: &mut Preferences) -> Result<(), String> {
        preferences.action_after_launch =
            ActionAfterLaunch::from_index(self.selected.launch_action)
                .ok_or_else(|| "Choose a valid launch action".to_owned())?;
        preferences.default_repeat_mode = self.repeat_mode;
        preferences.pause_when_open = self.selected.pause_when_open;
        preferences.play_when_entering_fullscreen = self.selected.play_when_entering_fullscreen;
        preferences.pause_when_leaving_fullscreen = self.selected.pause_when_leaving_fullscreen;
        preferences.pause_when_inactive = self.selected.pause_when_inactive;
        preferences.pause_when_minimized = self.selected.pause_when_minimized;
        preferences.pause_when_goes_to_sleep = self.selected.pause_when_goes_to_sleep;
        preferences.quit_when_no_opened_window = self.selected.quit_when_no_opened_window;
        preferences.keep_open_on_file_end = self.selected.keep_open_on_file_end;
        preferences.playlist_auto_play_next = self.selected.playlist_auto_play_next;
        preferences.playlist_auto_add = self.selected.playlist_auto_add;
        preferences.playlist_show_metadata = self.selected.playlist_show_metadata;
        preferences.playlist_show_metadata_in_music_mode =
            self.selected.playlist_show_metadata_in_music_mode;
        preferences.auto_repeat = self.selected.auto_repeat;
        preferences.fullscreen_when_open = self.selected.fullscreen_when_open;
        preferences.resume_last_position = self.selected.resume_last_position;
        preferences.record_playback_history = self.selected.record_playback_history;
        Ok(())
    }
}

impl Policy {
    fn from_player(app: &PlayerWindow) -> Self {
        let player_preferences = app.global::<riina_ui::PlayerPreferences>();
        Self {
            pause_when_open: player_preferences.get_pause_when_open(),
            play_when_entering_fullscreen: player_preferences.get_play_when_entering_fullscreen(),
            pause_when_leaving_fullscreen: player_preferences.get_pause_when_leaving_fullscreen(),
            pause_when_inactive: player_preferences.get_pause_when_inactive(),
            pause_when_minimized: player_preferences.get_pause_when_minimized(),
            pause_when_goes_to_sleep: player_preferences.get_pause_when_goes_to_sleep(),
            quit_when_no_opened_window: player_preferences.get_quit_when_no_opened_window(),
            keep_open_on_file_end: player_preferences.get_keep_open_on_file_end(),
            playlist_auto_play_next: player_preferences.get_playlist_auto_play_next(),
            playlist_auto_add: player_preferences.get_playlist_auto_add(),
            playlist_show_metadata: player_preferences.get_playlist_show_metadata(),
            playlist_show_metadata_in_music_mode: player_preferences
                .get_playlist_show_metadata_in_music_mode(),
            auto_repeat: player_preferences.get_auto_repeat(),
            default_repeat_mode: player_preferences.get_default_repeat_mode_index(),
            fullscreen_when_open: player_preferences.get_fullscreen_when_open(),
            resume_last_position: player_preferences.get_resume_last_position(),
            record_playback_history: player_preferences.get_record_playback_history(),
            launch_action: player_preferences.get_launch_action_index(),
        }
    }

    fn from_window(window: &PreferencesWindow) -> Self {
        Self {
            pause_when_open: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_pause_when_open(),
            play_when_entering_fullscreen: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_play_when_entering_fullscreen(),
            pause_when_leaving_fullscreen: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_pause_when_leaving_fullscreen(),
            pause_when_inactive: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_pause_when_inactive(),
            pause_when_minimized: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_pause_when_minimized(),
            pause_when_goes_to_sleep: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_pause_when_goes_to_sleep(),
            quit_when_no_opened_window: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_quit_when_no_opened_window(),
            keep_open_on_file_end: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_keep_open_on_file_end(),
            playlist_auto_play_next: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_playlist_auto_play_next(),
            playlist_auto_add: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_playlist_auto_add(),
            playlist_show_metadata: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_playlist_show_metadata(),
            playlist_show_metadata_in_music_mode: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_playlist_show_metadata_in_music_mode(),
            auto_repeat: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_auto_repeat(),
            default_repeat_mode: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_default_repeat_mode_index(),
            fullscreen_when_open: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_fullscreen_when_open(),
            resume_last_position: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_resume_last_position(),
            record_playback_history: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_record_playback_history(),
            launch_action: window
                .global::<riina_ui::GeneralSettingsState>()
                .get_launch_action_index(),
        }
    }

    pub(super) fn project_open_policy(&self, app: &PlayerWindow) {
        let player_preferences = app.global::<riina_ui::PlayerPreferences>();
        player_preferences.set_pause_when_open(self.pause_when_open);
        player_preferences.set_fullscreen_when_open(self.fullscreen_when_open);
        player_preferences.set_resume_last_position(self.resume_last_position);
        player_preferences.set_record_playback_history(self.record_playback_history);
        player_preferences.set_launch_action_index(self.launch_action.clamp(0, 2));
    }

    pub(super) fn restore(&self, app: &PlayerWindow) {
        let player_preferences = app.global::<riina_ui::PlayerPreferences>();
        player_preferences.set_pause_when_open(self.pause_when_open);
        player_preferences.set_play_when_entering_fullscreen(self.play_when_entering_fullscreen);
        player_preferences.set_pause_when_leaving_fullscreen(self.pause_when_leaving_fullscreen);
        player_preferences.set_pause_when_inactive(self.pause_when_inactive);
        player_preferences.set_pause_when_minimized(self.pause_when_minimized);
        player_preferences.set_pause_when_goes_to_sleep(self.pause_when_goes_to_sleep);
        player_preferences.set_quit_when_no_opened_window(self.quit_when_no_opened_window);
        player_preferences.set_keep_open_on_file_end(self.keep_open_on_file_end);
        player_preferences.set_playlist_auto_play_next(self.playlist_auto_play_next);
        player_preferences.set_playlist_auto_add(self.playlist_auto_add);
        player_preferences.set_playlist_show_metadata(self.playlist_show_metadata);
        player_preferences
            .set_playlist_show_metadata_in_music_mode(self.playlist_show_metadata_in_music_mode);
        player_preferences.set_auto_repeat(self.auto_repeat);
        player_preferences.set_default_repeat_mode_index(self.default_repeat_mode);
        player_preferences.set_fullscreen_when_open(self.fullscreen_when_open);
        player_preferences.set_resume_last_position(self.resume_last_position);
        player_preferences.set_record_playback_history(self.record_playback_history);
        player_preferences.set_launch_action_index(self.launch_action);
    }
}
