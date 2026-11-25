//! Edit subtitle preferences while preserving the decoder's accepted state.
use slint::ComponentHandle;

use super::read_or_refresh;
use crate::{
    PlayerWindow, PreferencesWindow,
    preferences::model::{PreferenceModel, PreferenceSection},
    preferences_window::live::{
        apply_ass_override, apply_subtitle_text_preferences, apply_text_style,
        restore_ass_override, restore_subtitle_text_preferences, restore_text_style,
    },
};
use riina_core::{
    Command, Preferences,
    subtitle_controls::{
        AssOverrideLevel, AutoLoadPreferences, OtherPreferences, PositionPreferences,
        TextStylePreferences,
    },
};
use riina_mpv::CommandSender;

pub(super) struct Draft {
    previous_font: String,
    saved_font: String,
    previous_size: f64,
    previous_style: TextStylePreferences,
    previous_overrides: (i32, i32),
    previous_position: PositionPreferences,
    position: PositionPreferences,
    previous_other: OtherPreferences,
    other: OtherPreferences,
    previous_auto_load: AutoLoadPreferences,
    auto_load: AutoLoadPreferences,
}

pub(super) struct Applied {
    pub(super) font: String,
    applied_font: String,
    pub(super) font_size: f64,
    pub(super) style: TextStylePreferences,
    pub(super) primary: AssOverrideLevel,
    pub(super) secondary: AssOverrideLevel,
    selected_overrides: (i32, i32),
}

impl Draft {
    pub(super) fn read(
        app: &PlayerWindow,
        window: &PreferencesWindow,
        model: &PreferenceModel,
        saved: &Preferences,
    ) -> Option<Self> {
        Some(Self {
            previous_font: app
                .global::<riina_ui::PlaybackSubtitleState>()
                .get_subtitle_font()
                .to_string(),
            saved_font: saved.subtitle_font.clone(),
            previous_size: crate::subtitle_preferences::current_font_size(app),
            previous_style: read_or_refresh(
                crate::subtitle_preferences::current_style(app),
                app,
                window,
            )?,
            previous_overrides: (
                app.global::<riina_ui::PlayerPreferences>()
                    .get_subtitle_ass_override_index(),
                app.global::<riina_ui::PlayerPreferences>()
                    .get_secondary_subtitle_ass_override_index(),
            ),
            previous_position: read_or_refresh(
                crate::subtitle_position_preferences::current_player(app),
                app,
                window,
            )?,
            position: read_or_refresh(
                model.read_section::<PositionPreferences>(window),
                app,
                window,
            )?,
            previous_other: read_or_refresh(
                crate::subtitle_other_preferences::current_player(app),
                app,
                window,
            )?,
            other: read_or_refresh(model.read_section::<OtherPreferences>(window), app, window)?,
            previous_auto_load: read_or_refresh(
                crate::subtitle_auto_load_preferences::current_player(app),
                app,
                window,
            )?,
            auto_load: read_or_refresh(
                model.read_section::<AutoLoadPreferences>(window),
                app,
                window,
            )?,
        })
    }

    pub(super) fn apply(
        &self,
        app: &PlayerWindow,
        window: &PreferencesWindow,
        sender: &CommandSender,
    ) -> Result<Applied, String> {
        let selected_overrides = (
            window
                .global::<crate::SubtitleSettingsState>()
                .get_subtitle_ass_override_primary_index(),
            window
                .global::<crate::SubtitleSettingsState>()
                .get_subtitle_ass_override_secondary_index(),
        );
        let selected_font = window
            .global::<crate::SubtitleSettingsState>()
            .get_subtitle_font();
        let font = crate::font_picker::font_name(selected_font)?;
        // An unrelated edit leaves a temporary player font in place.
        let live_font = if font == self.saved_font {
            &self.previous_font
        } else {
            &font
        };
        let size = crate::subtitle_preferences::font_size(
            window
                .global::<crate::SubtitleSettingsState>()
                .get_subtitle_font_size_text()
                .as_str(),
        )?;
        let (applied_font, font_size) = apply_subtitle_text_preferences(
            app,
            sender,
            &self.previous_font,
            self.previous_size,
            live_font,
            size,
        )?;
        let restore_font = || {
            restore_subtitle_text_preferences(
                app,
                sender,
                &self.previous_font,
                self.previous_size,
                &applied_font,
                font_size,
            );
        };
        let style = crate::subtitle_preferences::current_window_style(window)
            .inspect_err(|_| restore_font())?;
        if let Err(error) = apply_text_style(app, sender, self.previous_style, style) {
            restore_font();
            return Err(error);
        }
        let (primary, secondary) =
            match apply_ass_override(app, sender, self.previous_overrides, selected_overrides) {
                Ok(levels) => levels,
                Err(error) => {
                    restore_text_style(app, sender, self.previous_style, style);
                    restore_font();
                    return Err(error);
                }
            };
        Ok(Applied {
            font,
            applied_font,
            font_size,
            style,
            primary,
            secondary,
            selected_overrides,
        })
    }

    pub(super) fn apply_tracks(&self, sender: &CommandSender) -> Result<(), String> {
        crate::subtitle_position_preferences::apply(sender, self.previous_position, self.position)?;
        crate::subtitle_other_preferences::apply(sender, &self.previous_other, &self.other)?;
        sender
            .send(Command::SubtitleAutoLoad(self.auto_load.clone()))
            .map_err(String::from)
    }

    pub(super) fn project_tracks(&self, app: &PlayerWindow) {
        self.position.write_player(app);
        self.other.write_player(app);
        self.auto_load.write_player(app);
    }

    pub(super) fn put_tracks(&self, preferences: &mut Preferences) {
        self.auto_load.clone().put(preferences);
        self.other.clone().put(preferences);
        self.position.put(preferences);
    }

    pub(super) fn restore_tracks(&self, app: &PlayerWindow, sender: &CommandSender) {
        crate::preferences::restore_playback(
            sender,
            Command::SubtitleAutoLoad(self.previous_auto_load.clone()),
        );
        if let Err(error) =
            crate::subtitle_other_preferences::apply(sender, &self.other, &self.previous_other)
        {
            crate::diagnostics::warn!("Could not restore the previous subtitle settings: {error}");
        }
        self.previous_other.write_player(app);
        self.previous_auto_load.write_player(app);
        if let Err(error) = crate::subtitle_position_preferences::apply(
            sender,
            self.position,
            self.previous_position,
        ) {
            crate::diagnostics::warn!("Could not restore the previous subtitle position: {error}");
        }
        self.previous_position.write_player(app);
    }
}

impl Applied {
    pub(super) fn restore(&self, app: &PlayerWindow, sender: &CommandSender, draft: &Draft) {
        restore_text_style(app, sender, draft.previous_style, self.style);
        restore_subtitle_text_preferences(
            app,
            sender,
            &draft.previous_font,
            draft.previous_size,
            &self.applied_font,
            self.font_size,
        );
        restore_ass_override(
            app,
            sender,
            draft.previous_overrides,
            self.selected_overrides,
        );
    }
}
