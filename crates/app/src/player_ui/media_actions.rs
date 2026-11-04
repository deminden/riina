//! File, URL, recent-document and welcome-window actions share one command sender.
use super::{
    PendingWelcomeOpen, dispatch_welcome_open, help_menu_url, persist_recent_open_documents,
    remember_recent_open_document, set_native_video_host_visible,
};
use crate::{
    MenuCatalogRow, NativeVideoHost, PlayerWindow, WelcomeWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Playback, RecentMedia};
use riina_mpv::Player;
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

pub(super) struct Context<'a> {
    pub player: &'a Player,
    pub open_host: Option<slint::Weak<NativeVideoHost>>,
    pub welcome: Option<&'a WelcomeWindow>,
    pub recent_media: &'a Rc<RefCell<Vec<RecentMedia>>>,
    pub recent_open_documents: &'a Rc<RefCell<Vec<RecentMedia>>>,
    pub recent_open_cleared: &'a Rc<Cell<bool>>,
    pub prefs_path: &'a PathBuf,
    pub latest_playback: &'a Arc<Mutex<Playback>>,
}

pub(super) fn install(app: &PlayerWindow, context: Context<'_>) {
    let Context {
        player,
        open_host,
        welcome,
        recent_media,
        recent_open_documents,
        recent_open_cleared,
        prefs_path,
        latest_playback,
    } = context;
    let open_player = player.command_sender();
    let weak = app.as_weak();
    let welcome_window = welcome.map(|welcome| welcome.as_weak());
    let pending_welcome_open = Rc::new(RefCell::new(None));
    let ready_pending = pending_welcome_open.clone();
    let ready_app = app.as_weak();
    let ready_player = player.command_sender();
    app.on_renderer_ready(move || {
        if let Some(request) = ready_pending.borrow_mut().take()
            && let Some(app) = ready_app.upgrade()
        {
            dispatch_welcome_open(&app, request, &ready_player);
        }
    });
    let file_welcome = welcome_window.clone();
    let file_open_host = open_host.clone();
    let file_dialog_host = open_host.clone();
    let new_window_app = app.as_weak();
    let new_window_host = open_host.clone();
    let new_window_sender = open_player.clone();
    let reveal_media_app = app.as_weak();
    let reveal_media_playback = latest_playback.clone();
    app.on_reveal_current_media(move || {
        if let Some(app) = reveal_media_app.upgrade() {
            crate::key_binding_preferences::reveal_current_media(&app, &reveal_media_playback);
        }
    });
    let save_playlist_app = app.as_weak();
    let save_playlist_host = open_host.clone();
    let save_playlist_playback = latest_playback.clone();
    app.on_save_current_playlist(move || {
        if let Some(app) = save_playlist_app.upgrade() {
            crate::playlist_export::save_current_playlist(
                &app,
                save_playlist_host.clone(),
                &save_playlist_playback,
            );
        }
    });
    let save_subtitle_app = app.as_weak();
    let save_subtitle_host = open_host.clone();
    let save_subtitle_playback = latest_playback.clone();
    app.on_save_downloaded_subtitle(move || {
        if let Some(app) = save_subtitle_app.upgrade() {
            crate::file_actions::save_downloaded_subtitle(
                &app,
                save_subtitle_host.clone(),
                &save_subtitle_playback,
            );
        }
    });
    let delete_media_app = app.as_weak();
    let delete_media_playback = latest_playback.clone();
    let delete_media_player = open_player.clone();
    app.on_delete_current_media(move || {
        if let Some(app) = delete_media_app.upgrade() {
            crate::file_actions::delete_current_media(
                &app,
                &delete_media_playback,
                delete_media_player.clone(),
            );
        }
    });
    app.on_open_in_new_window(move || {
        if let Some(app) = new_window_app.upgrade() {
            ui_dialogs::open(
                &app,
                new_window_host.clone(),
                Purpose::MediaInNewPlayer,
                new_window_sender.clone(),
                None,
            );
        }
    });
    app.on_open_file(move || {
        if let Some(app) = weak.upgrade() {
            let welcome_is_visible = file_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if welcome_is_visible {
                set_native_video_host_visible(file_open_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = file_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    let _ = welcome.hide();
                }
            }
            ui_dialogs::open(
                &app,
                file_dialog_host.clone(),
                Purpose::Media,
                open_player.clone(),
                welcome_is_visible.then(|| file_welcome.clone()).flatten(),
            );
        }
    });
    let remember_documents = recent_open_documents.clone();
    let remember_preferences_path = prefs_path.clone();
    let remember_app = app.as_weak();
    app.on_remember_recent_path(move |path| {
        if let Some(app) = remember_app.upgrade() {
            let title = PathBuf::from(path.as_str()).file_name().map_or_else(
                || path.to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            remember_recent_open_document(
                &app,
                &remember_documents,
                &remember_preferences_path,
                path.to_string(),
                title,
            );
        }
    });
    let open_recent_documents = recent_open_documents.clone();
    let open_recent_preferences_path = prefs_path.clone();
    let recent_player = player.command_sender();
    let open_recent_app = app.as_weak();
    app.on_open_recent_path(move |path| {
        let Some(app) = open_recent_app.upgrade() else {
            return;
        };
        let Some(media) = open_recent_documents
            .borrow()
            .iter()
            .find(|item| item.path == path.as_str())
            .cloned()
        else {
            return;
        };
        remember_recent_open_document(
            &app,
            &open_recent_documents,
            &open_recent_preferences_path,
            media.path.clone(),
            media.title.clone(),
        );
        media_open::open_recent(
            &app,
            media,
            app.get_resume_last_position(),
            recent_player.clone(),
        );
    });
    let clear_recent_documents = recent_open_documents.clone();
    let clear_recent_flag = recent_open_cleared.clone();
    let clear_preferences_path = prefs_path.clone();
    let clear_recent_app = app.as_weak();
    app.on_clear_open_recent(move || {
        if let Some(app) = clear_recent_app.upgrade() {
            clear_recent_flag.set(true);
            clear_recent_documents.borrow_mut().clear();
            app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(Vec::<
                MenuCatalogRow,
            >::new(
            ))));
            app.invoke_video_menu_changed();
            if let Err(error) = persist_recent_open_documents(
                &clear_preferences_path,
                &clear_recent_documents.borrow(),
            ) {
                app.set_status(format!("Could not clear Open Recent: {error}").into());
            }
            if let Err(error) = platform::clear_recent_documents() {
                app.set_status(format!("Could not clear system recent documents: {error}").into());
            }
        }
    });
    let open_url = player.command_sender();
    let weak = app.as_weak();
    let open_url_host = open_host.clone();
    let url_welcome = welcome_window.clone();
    let url_pending = pending_welcome_open.clone();
    app.on_open_url(move |value| {
        if let Some(app) = weak.upgrade() {
            let from_welcome = url_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if from_welcome {
                if let Err(error) = media_open::validate_url(value.as_str()) {
                    if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                        welcome.set_url_error(error.into());
                    }
                    return;
                }
                set_native_video_host_visible(open_url_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    welcome.set_url_error("".into());
                    let _ = welcome.hide();
                }
                let request = PendingWelcomeOpen::Url(value.to_string());
                if app.get_engine_ready() {
                    dispatch_welcome_open(&app, request, &open_url);
                } else {
                    *url_pending.borrow_mut() = Some(request);
                }
            } else {
                media_open::url(&app, value.as_str(), open_url.clone());
            }
        }
    });
    let weak = app.as_weak();
    app.on_open_help_link(move |destination| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let Some(url) = help_menu_url(destination.as_str()) else {
            return;
        };
        if let Err(error) = crate::platform::open_http_url_in_browser(url) {
            app.set_status(format!("Could not open the Help link: {error}").into());
        }
    });
    let weak = app.as_weak();
    let debug_playback = latest_playback.clone();
    app.on_dump_debug_info(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let snapshot = match debug_playback.lock() {
            Ok(playback) => playback.clone(),
            Err(_) => {
                app.set_status("Could not read player state for the debug dump".into());
                return;
            }
        };
        match crate::debug_dump::write_download(&snapshot) {
            Ok(path) => {
                let _ = rfd::MessageDialog::new()
                    .set_title("Debug Info Saved")
                    .set_description(format!(
                        "Debug information was saved to {}. The file may contain media filenames and URLs. Review it before sharing.",
                        path.display()
                    ))
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
            }
            Err(error) => app.set_status(format!("Could not save debug information: {error}").into()),
        }
    });
    let recent_player = player.command_sender();
    let recent_host = open_host.clone();
    let recent_media_for_open = recent_media.clone();
    let weak = app.as_weak();
    let recent_welcome = welcome_window.clone();
    let recent_pending = pending_welcome_open.clone();
    app.on_open_recent(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let media = recent_media_for_open.borrow().get(index).cloned();
        if let (Some(app), Some(media)) = (weak.upgrade(), media) {
            let from_welcome = recent_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if from_welcome {
                set_native_video_host_visible(recent_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = recent_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    let _ = welcome.hide();
                }
                let request = PendingWelcomeOpen::Recent(media, app.get_resume_last_position());
                if app.get_engine_ready() {
                    dispatch_welcome_open(&app, request, &recent_player);
                } else {
                    *recent_pending.borrow_mut() = Some(request);
                }
            } else {
                media_open::open_recent(
                    &app,
                    media,
                    app.get_resume_last_position(),
                    recent_player.clone(),
                );
            }
        }
    });
    let opened_documents = recent_open_documents.clone();
    let opened_preferences_path = prefs_path.clone();
    let opened_app = app.as_weak();
    let opened_welcome = welcome_window.clone();
    app.on_media_opened(move |path, title| {
        if let Some(app) = opened_app.upgrade()
            && app.get_track_all_files_in_recent_open_menu()
        {
            remember_recent_open_document(
                &app,
                &opened_documents,
                &opened_preferences_path,
                path.to_string(),
                title.to_string(),
            );
        }
        if let Some(welcome) = opened_welcome.as_ref().and_then(slint::Weak::upgrade) {
            let _ = welcome.hide();
        }
    });
}
