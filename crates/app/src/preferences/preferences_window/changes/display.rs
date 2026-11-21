//! Read, project and restore display settings during a preference edit.

use super::{read_or_refresh, refresh};
use crate::{
    PlayerWindow, PreferencesWindow,
    osd_preferences::Settings as Osd,
    preferences::model::{PreferenceModel, PreferenceSection},
};
use riina_core::{
    Preferences, ThemeMaterial, liquid_glass, osc_preferences,
    pip_preferences::Settings as PictureInPicture,
    thumbnail_preferences::Settings as ThumbnailPreview, window_resize,
};
use slint::ComponentHandle;

struct Layout {
    position: i32,
    compact: bool,
    docked: bool,
}

pub(super) struct Draft {
    previous_osd: Osd,
    osd: Osd,
    control_bar: osc_preferences::Settings,
    picture_in_picture: PictureInPicture,
    thumbnail_preview: ThumbnailPreview,
    previous_layout: Layout,
    previous_chapter_positions: bool,
    previous_resize: window_resize::Settings,
    pub(super) previous_volume_boost: bool,
    previous_material: liquid_glass::SurfaceSettings,
    material: liquid_glass::SurfaceSettings,
    theme: ThemeMaterial,
    resize: window_resize::Settings,
    dark: bool,
}

impl Draft {
    pub(super) fn read(
        app: &PlayerWindow,
        window: &PreferencesWindow,
        model: &PreferenceModel,
    ) -> Option<Self> {
        let previous_osd = crate::osd_preferences::current_player(app);
        let osd = read_or_refresh(model.read_section::<Osd>(window), app, window)?;
        let control_bar = read_or_refresh(
            model.read_section::<osc_preferences::Settings>(window),
            app,
            window,
        )?;
        let picture_in_picture =
            read_or_refresh(model.read_section::<PictureInPicture>(window), app, window)?;
        let thumbnail_preview =
            read_or_refresh(model.read_section::<ThumbnailPreview>(window), app, window)?;
        let previous_layout = Layout {
            position: app
                .global::<riina_ui::PlayerLayoutState>()
                .get_osc_position(),
            compact: app.global::<riina_ui::PlayerAppearance>().get_compact_ui(),
            docked: app
                .global::<riina_ui::PlayerLayoutState>()
                .get_docked_controls(),
        };
        let previous_chapter_positions = app
            .global::<riina_ui::PlayerPreferences>()
            .get_show_chapter_pos();
        let previous_resize = read_or_refresh(
            crate::window_resize_preferences::from_player(app),
            app,
            window,
        )?;
        let previous_volume_boost = app
            .global::<riina_ui::PlayerPreferences>()
            .get_volume_boost();
        let previous_material = crate::liquid_glass_preferences::current_player(app);
        let material = read_or_refresh(
            model.read_section::<liquid_glass::SurfaceSettings>(window),
            app,
            window,
        )?;
        let Some(theme) = ThemeMaterial::from_index(window.get_appearance_index()) else {
            app.set_status("Choose a valid appearance".into());
            refresh(window, app);
            return None;
        };
        let resize = read_or_refresh(
            model.read_section::<window_resize::Settings>(window),
            app,
            window,
        )?;
        let dark = theme.dark_for(app.global::<riina_ui::PlayerAppearance>().get_dark_theme());
        Some(Self {
            previous_osd,
            osd,
            control_bar,
            picture_in_picture,
            thumbnail_preview,
            previous_layout,
            previous_chapter_positions,
            previous_resize,
            previous_volume_boost,
            previous_material,
            material,
            theme,
            resize,
            dark,
        })
    }

    pub(super) fn project_before_save(&self, app: &PlayerWindow, window: &PreferencesWindow) {
        let player_preferences = app.global::<riina_ui::PlayerPreferences>();
        self.resize.write_player(app);
        player_preferences.set_osd_enabled(window.get_osd_enabled());
        player_preferences.set_disable_osd_speed(window.get_disable_osd_speed());
        player_preferences.set_disable_osd_seek(window.get_disable_osd_seek());
        app.global::<riina_ui::PlayerLayoutState>()
            .set_osc_position(window.get_osc_position().clamp(0, 2));
        player_preferences.set_show_chapter_pos(window.get_show_chapter_pos());
        app.global::<riina_ui::PlayerAppearance>()
            .set_compact_ui(window.get_compact_ui());
        if !app
            .global::<riina_ui::PlayerLayoutState>()
            .get_edge_to_edge_video()
        {
            app.global::<riina_ui::PlayerLayoutState>()
                .set_docked_controls(window.get_docked_controls());
        }
    }

    pub(super) fn put(&self, preferences: &mut Preferences) {
        self.thumbnail_preview.clone().put(preferences);
        self.picture_in_picture.put(preferences);
        self.osd.put(preferences);
        self.material.put(preferences);
        self.control_bar.put(preferences);
        self.resize.put(preferences);
        preferences.theme_material = Some(self.theme);
        preferences.dark = self.dark;
    }

    pub(super) fn restore(&self, app: &PlayerWindow) {
        self.previous_osd.install_player(app);
        app.global::<riina_ui::PlayerLayoutState>()
            .set_osc_position(self.previous_layout.position);
        app.global::<riina_ui::PlayerPreferences>()
            .set_show_chapter_pos(self.previous_chapter_positions);
        app.global::<riina_ui::PlayerAppearance>()
            .set_compact_ui(self.previous_layout.compact);
        app.global::<riina_ui::PlayerLayoutState>()
            .set_docked_controls(self.previous_layout.docked);
        self.previous_material.write_player(app);
        self.previous_resize.write_player(app);
    }
}
