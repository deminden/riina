//! Edit subtitle preferences while preserving the decoder's accepted state.

use super::read_or_refresh;
use crate::{
    PlayerWindow, PreferencesWindow,
    preferences::model::PreferenceModel,
    preferences_window::live::{
        apply_ass_override, apply_subtitle_text_preferences, apply_text_style,
        restore_ass_override, restore_subtitle_text_preferences, restore_text_style,
    },
};
use riina_core::subtitle_controls::{
    AssOverrideLevel, AutoLoadPreferences, OtherPreferences, PositionPreferences,
    TextStylePreferences,
};
use riina_mpv::CommandSender;

pub(super) struct Draft {
    previous_font: String,
    previous_size: f64,
    previous_style: TextStylePreferences,
    previous_overrides: (i32, i32),
    pub(super) previous_position: PositionPreferences,
    pub(super) position: PositionPreferences,
    pub(super) previous_other: OtherPreferences,
    pub(super) other: OtherPreferences,
    pub(super) previous_auto_load: AutoLoadPreferences,
    pub(super) auto_load: AutoLoadPreferences,
}

pub(super) struct Applied {
    pub(super) font: String,
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
    ) -> Option<Self> {
        Some(Self {
            previous_font: app.get_subtitle_font().to_string(),
            previous_size: crate::subtitle_preferences::current_font_size(app),
            previous_style: read_or_refresh(
                crate::subtitle_preferences::current_style(app),
                app,
                window,
            )?,
            previous_overrides: (
                app.get_subtitle_ass_override_index(),
                app.get_secondary_subtitle_ass_override_index(),
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
            window.get_subtitle_ass_override_primary_index(),
            window.get_subtitle_ass_override_secondary_index(),
        );
        let font = window.get_subtitle_font();
        let size =
            crate::subtitle_preferences::font_size(window.get_subtitle_font_size_text().as_str())?;
        let (font, font_size) = apply_subtitle_text_preferences(
            app,
            sender,
            &self.previous_font,
            self.previous_size,
            font.as_str(),
            size,
        )?;
        let restore_font = || {
            restore_subtitle_text_preferences(
                app,
                sender,
                &self.previous_font,
                self.previous_size,
                &font,
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
            font_size,
            style,
            primary,
            secondary,
            selected_overrides,
        })
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
            &self.font,
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
