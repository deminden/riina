//! The framed owner reads menu state from the controls immediately before opening.
#[cfg(test)]
mod tests;
use crate::{NativeVideoHost, PlayerWindow};
use slint::{ComponentHandle, Model};
use std::{cell::Cell, rc::Rc};

pub(super) fn install(host: &NativeVideoHost, app: &PlayerWindow) {
    let menu_state_host = host.as_weak();
    let menu_state_app = app.as_weak();
    app.on_native_menu_will_open(move || {
        if let (Some(host), Some(app)) = (menu_state_host.upgrade(), menu_state_app.upgrade()) {
            sync(&host, &app);
        }
    });
    // The framed owner hosts the native menu; its transparent popup
    // retains controller state. Bridge observations without a second
    // playback model or sending window handles across threads.
    let menu_host = host.as_weak();
    let menu_app = app.as_weak();
    let menu_pending = Rc::new(Cell::new(false));
    app.on_video_menu_changed(move || {
        if menu_pending.replace(true) {
            return;
        }
        let menu_host = menu_host.clone();
        let menu_app = menu_app.clone();
        let menu_pending = menu_pending.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(80), move || {
            menu_pending.set(false);
            if let (Some(host), Some(app)) = (menu_host.upgrade(), menu_app.upgrade()) {
                sync(&host, &app);
            }
        });
    });
    app.invoke_video_menu_changed();
}

fn sync(host: &NativeVideoHost, app: &PlayerWindow) {
    let settings_pane = if app.get_settings_at_leading() {
        app.get_leading_sidebar()
    } else {
        app.get_sidebar()
    };
    host.set_video_panel_open(settings_pane == 6);
    host.set_audio_panel_open(settings_pane == 1);
    host.set_subtitles_panel_open(settings_pane == 2);
    host.set_plugins_panel_open(app.get_leading_sidebar() == 7 || app.get_sidebar() == 7);
    host.set_reflection_menu_enabled(
        app.get_loaded() && app.get_reflection_known() && !app.get_crop_editing(),
    );
    host.set_video_mirrored(app.get_video_mirrored());
    host.set_video_flipped(app.get_video_flipped());
    host.set_delogo_menu_enabled(app.get_delogo_enabled() && !app.get_crop_editing());
    host.set_delogo_active(app.get_delogo_active());
    host.set_saved_video_filter_rows(app.get_saved_video_filter_rows());
    host.set_saved_audio_filter_rows(app.get_saved_audio_filter_rows());
    host.set_crop_menu_enabled(
        app.get_loaded() && app.get_video_crop_known() && !app.get_crop_editing(),
    );
    host.set_crop_menu_labels(app.get_video_crop_menu_labels());
    host.set_crop_menu_index(app.get_video_crop_menu_index());
    host.set_aspect_menu_enabled(
        app.get_loaded() && app.get_video_aspect_known() && !app.get_crop_editing(),
    );
    host.set_aspect_menu_labels(app.get_video_aspect_menu_labels());
    host.set_video_aspect_menu_index(app.get_video_aspect_menu_index());
    host.set_rotation_menu_enabled(
        app.get_loaded() && app.get_video_rotation_known() && !app.get_crop_editing(),
    );
    host.set_video_rotation_index(app.get_video_rotation_index());
    host.set_deinterlace_menu_enabled(
        app.get_loaded() && app.get_deinterlace_known() && !app.get_crop_editing(),
    );
    host.set_deinterlace_enabled(app.get_deinterlace_enabled());
    host.set_screenshot_available(app.get_screenshot_available() && !app.get_crop_editing());
    host.set_screenshot_folder_enabled(!app.get_crop_editing());
    host.set_fullscreen_mode(app.get_fullscreen());
    host.set_always_float_on_top(app.get_always_float_on_top());
    host.set_menu_shortcuts(app.get_menu_shortcuts());
    host.set_unlock_window_aspect_ratio(app.get_unlock_window_aspect_ratio());
    host.set_live_text_available(app.get_live_text_available());
    host.set_live_text_enabled(app.get_live_text_enabled());
    host.set_record_recent_files(app.get_record_recent_files());
    host.set_recent_open_menu_rows(app.get_recent_open_menu_rows());
    host.set_plugin_menu_rows(app.get_plugin_menu_rows());
    host.set_playback_loaded(app.get_loaded() && !app.get_crop_editing());
    host.set_reveal_current_media_available(app.get_reveal_current_media_available());
    host.set_delete_current_media_available(app.get_delete_current_media_available());
    host.set_save_current_playlist_available(app.get_save_current_playlist_available());
    host.set_downloaded_subtitle_save_available(app.get_downloaded_subtitle_save_available());
    host.set_open_subtitles_provider_index(app.get_open_subtitles_provider_index());
    host.set_current_file_manager_name(app.get_current_file_manager_name());
    host.set_playback_paused(app.get_paused());
    host.set_menu_alternates_visible(app.get_menu_alternates_visible());
    host.set_playback_muted(app.get_muted());
    host.set_loop_label(app.get_loop_label());
    let playlist_rows = app.get_playlist();
    crate::ui_models::set_rows_if_changed(
        &host.get_playlist_menu_rows(),
        (0..playlist_rows.row_count())
            .filter_map(|index| playlist_rows.row_data(index))
            .map(|row| crate::MenuCatalogRow {
                token: row.token,
                label: row.label,
                selected: row.playing,
            })
            .collect::<Vec<_>>(),
        |rows| host.set_playlist_menu_rows(rows),
    );
    host.set_audio_device_menu_rows(app.get_audio_device_menu_rows());
    let audio_rows = app.get_audio_tracks();
    crate::ui_models::set_rows_if_changed(
        &host.get_audio_track_menu_rows(),
        (0..audio_rows.row_count())
            .filter_map(|index| audio_rows.row_data(index))
            .map(|row| crate::MenuCatalogRow {
                token: row.token,
                label: row.label,
                selected: row.selected,
            })
            .collect(),
        |rows| host.set_audio_track_menu_rows(rows),
    );
    let chapter_rows = app.get_chapters();
    crate::ui_models::set_rows_if_changed(
        &host.get_chapter_menu_rows(),
        (0..chapter_rows.row_count())
            .filter_map(|index| chapter_rows.row_data(index))
            .map(|row| crate::MenuCatalogRow {
                token: row.token,
                label: row.label,
                selected: row.selected,
            })
            .collect::<Vec<_>>(),
        |rows| host.set_chapter_menu_rows(rows),
    );
    let video_rows = app.get_video_tracks();
    crate::ui_models::set_rows_if_changed(
        &host.get_video_track_menu_rows(),
        (0..video_rows.row_count())
            .filter_map(|index| video_rows.row_data(index))
            .map(|row| crate::MenuCatalogRow {
                token: row.token,
                label: row.label,
                selected: row.selected,
            })
            .collect::<Vec<_>>(),
        |rows| host.set_video_track_menu_rows(rows),
    );
    host.set_subtitle_visible(app.get_subtitle_visible());
    host.set_secondary_subtitle_visible(app.get_secondary_subtitle_visible());
    host.set_secondary_subtitle_visibility_known(app.get_secondary_subtitle_visibility_known());
    host.set_subtitle_scale_known(app.get_subtitle_scale_known());
    host.set_subtitle_default_encoding(app.get_subtitle_default_encoding());
    let subtitle_rows = app.get_subtitle_tracks();
    crate::ui_models::set_rows_if_changed(
        &host.get_subtitle_track_menu_rows(),
        (0..subtitle_rows.row_count())
            .filter_map(|index| subtitle_rows.row_data(index))
            .map(|row| crate::MenuCatalogRow {
                token: row.token,
                label: row.label,
                selected: row.selected,
            })
            .collect::<Vec<_>>(),
        |rows| host.set_subtitle_track_menu_rows(rows),
    );
    let secondary_subtitle_rows = app.get_secondary_subtitle_tracks();
    crate::ui_models::set_rows_if_changed(
        &host.get_secondary_subtitle_menu_rows(),
        (0..secondary_subtitle_rows.row_count())
            .filter_map(|index| secondary_subtitle_rows.row_data(index))
            .map(|row| crate::MenuCatalogRow {
                token: row.token,
                label: row.label,
                selected: row.selected,
            })
            .collect::<Vec<_>>(),
        |rows| host.set_secondary_subtitle_menu_rows(rows),
    );
    let subtitle_encodings = app.get_subtitle_encoding_options();
    crate::ui_models::set_rows_if_changed(
        &host.get_subtitle_encoding_menu_rows(),
        (0..subtitle_encodings.row_count())
            .filter_map(|index| subtitle_encodings.row_data(index))
            .map(|row| crate::MenuCatalogRow {
                token: row.token,
                label: row.label,
                selected: row.selected,
            })
            .collect::<Vec<_>>(),
        |rows| host.set_subtitle_encoding_menu_rows(rows),
    );
    host.set_music_mode_available(app.get_loaded() && !app.get_crop_editing());
    host.set_music_mode(app.get_music_mode());
    host.set_video_resize_available(
        app.get_video_window_width() > 0.0
            && app.get_video_window_height() > 0.0
            && !app.get_crop_editing(),
    );
    host.set_picture_in_picture_available(
        app.get_loaded() && app.get_video_width() > 0.0 && !app.get_crop_editing(),
    );
}
