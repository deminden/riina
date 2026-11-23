//! File, URL, recent-document and welcome-window actions share one command sender.
pub(super) mod recent;

use super::help_menu_url;
use crate::{
    MenuCatalogRow, NativeVideoHost, PlayerWindow, WelcomeWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use recent::{persist_recent_open_documents, remember_recent_open_document};
use riina_core::{Command, RecentMedia};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

pub(super) struct Context<'a> {
    pub commands: &'a CommandSender,
    pub open_host: Option<slint::Weak<NativeVideoHost>>,
    pub welcome: Option<&'a WelcomeWindow>,
    pub recent_media: &'a Rc<RefCell<Vec<RecentMedia>>>,
    pub recent_open_documents: &'a Rc<RefCell<Vec<RecentMedia>>>,
    pub recent_open_cleared: &'a Rc<Cell<bool>>,
    pub prefs_path: &'a PathBuf,
    pub latest_playback: &'a crate::PlaybackStore,
}

enum PendingWelcomeOpen {
    Url(String),
    Recent(riina_core::RecentMedia, bool),
}

fn dispatch_welcome_open(
    app: &PlayerWindow,
    request: PendingWelcomeOpen,
    sender: &riina_mpv::CommandSender,
) {
    match request {
        PendingWelcomeOpen::Url(value) => media_open::url(app, &value, sender.clone()),
        PendingWelcomeOpen::Recent(media, resume) => {
            media_open::open_recent(app, media, resume, sender.clone());
        }
    }
}

type WelcomeHandle = Option<slint::Weak<WelcomeWindow>>;
type OpenHost = Option<slint::Weak<NativeVideoHost>>;
type WelcomeRequests = Rc<RefCell<Option<PendingWelcomeOpen>>>;

pub(super) fn install(app: &PlayerWindow, context: Context<'_>) {
    let welcome_window = context.welcome.map(ComponentHandle::as_weak);
    let pending_welcome_open = Rc::new(RefCell::new(None));
    install_renderer_ready(app, context.commands, &pending_welcome_open);
    install_external_file_actions(
        app,
        context.commands,
        &context.open_host,
        context.latest_playback,
    );
    install_file_open(app, context.commands, &context.open_host, &welcome_window);
    install_open_recent_menu(app, &context);
    install_url_open(app, &context, &welcome_window, &pending_welcome_open);
    install_help_and_debug(app, context.latest_playback);
    install_welcome_recent(app, &context, &welcome_window, &pending_welcome_open);
    install_media_opened(app, &context, &welcome_window);
}

pub(super) fn install_resume_preference(
    app: &PlayerWindow,
    commands: &CommandSender,
    welcome: Option<&WelcomeWindow>,
    recent_media: &Rc<RefCell<Vec<RecentMedia>>>,
) {
    let resume_player = commands.clone();
    let resume_app = app.as_weak();
    let resume_welcome = welcome.map(WelcomeWindow::as_weak);
    let recent_media_for_resume = recent_media.clone();
    app.on_resume_last_position_changed(move |enabled| {
        if let Err(error) = resume_player.send(Command::SetResumeLastPosition(enabled))
            && let Some(app) = resume_app.upgrade()
        {
            crate::diagnostics::report(&app, error);
        }
        if let Some(welcome) = resume_welcome.as_ref().and_then(slint::Weak::upgrade) {
            recent::refresh_welcome_history(&welcome, &recent_media_for_resume.borrow(), enabled);
        }
    });
}

pub(super) fn install_pause_on_open(app: &PlayerWindow, commands: &CommandSender) {
    let opening_player = commands.clone();
    let opening_app = app.as_weak();
    app.on_pause_when_open_changed(move |pause| {
        if let Err(error) = opening_player.send(Command::SetPauseWhenOpen(pause))
            && let Some(app) = opening_app.upgrade()
        {
            crate::diagnostics::report(&app, error);
        }
    });
}

fn install_renderer_ready(
    app: &PlayerWindow,
    commands: &CommandSender,
    pending_welcome_open: &WelcomeRequests,
) {
    let ready_pending = pending_welcome_open.clone();
    let ready_app = app.as_weak();
    let ready_player = commands.clone();
    app.on_renderer_ready(move || {
        if let Some(request) = ready_pending.borrow_mut().take()
            && let Some(app) = ready_app.upgrade()
        {
            dispatch_welcome_open(&app, request, &ready_player);
        }
    });
}

fn install_external_file_actions(
    app: &PlayerWindow,
    commands: &CommandSender,
    open_host: &OpenHost,
    latest_playback: &crate::PlaybackStore,
) {
    let reveal_media_app = app.as_weak();
    let reveal_media_playback = latest_playback.clone();
    app.global::<PlayerMenuActions>()
        .on_reveal_current_media(move || {
            if let Some(app) = reveal_media_app.upgrade() {
                crate::key_binding_preferences::reveal_current_media(&app, &reveal_media_playback);
            }
        });
    let save_playlist_app = app.as_weak();
    let save_playlist_host = open_host.clone();
    let save_playlist_playback = latest_playback.clone();
    app.global::<PlayerMenuActions>()
        .on_save_current_playlist(move || {
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
    app.global::<PlayerMenuActions>()
        .on_save_downloaded_subtitle(move || {
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
    let delete_media_player = commands.clone();
    app.global::<PlayerMenuActions>()
        .on_delete_current_media(move || {
            if let Some(app) = delete_media_app.upgrade() {
                crate::file_actions::delete_current_media(
                    &app,
                    &delete_media_playback,
                    delete_media_player.clone(),
                );
            }
        });
}

fn install_file_open(
    app: &PlayerWindow,
    commands: &CommandSender,
    open_host: &OpenHost,
    welcome_window: &WelcomeHandle,
) {
    let open_player = commands.clone();
    let weak = app.as_weak();
    let file_welcome = welcome_window.clone();
    let file_open_host = open_host.clone();
    let file_dialog_host = open_host.clone();
    let new_window_app = app.as_weak();
    let new_window_host = open_host.clone();
    let new_window_sender = open_player.clone();
    app.global::<PlayerMenuActions>()
        .on_open_in_new_window(move || {
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
    app.global::<PlayerMenuActions>().on_open_file(move || {
        if let Some(app) = weak.upgrade() {
            let welcome_is_visible = file_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if welcome_is_visible {
                platform::set_native_video_host_visible(file_open_host.as_ref(), true);
                if let Err(error) = platform::show_window(&app) {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = file_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    // Continue releasing session resources if the window is closing.
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
}

fn install_open_recent_menu(app: &PlayerWindow, context: &Context<'_>) {
    let Context {
        commands,
        recent_open_documents,
        prefs_path,
        recent_open_cleared,
        ..
    } = *context;
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
    let recent_player = commands.clone();
    let open_recent_app = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_open_recent_path(move |path| {
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
                app.global::<riina_ui::PlayerPreferences>()
                    .get_resume_last_position(),
                recent_player.clone(),
            );
        });
    let clear_recent_documents = recent_open_documents.clone();
    let clear_recent_flag = recent_open_cleared.clone();
    let clear_preferences_path = prefs_path.clone();
    let clear_recent_app = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_clear_open_recent(move || {
            if let Some(app) = clear_recent_app.upgrade() {
                clear_recent_flag.set(true);
                clear_recent_documents.borrow_mut().clear();
                app.global::<riina_ui::PlayerCatalogState>()
                    .set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(Vec::<
                        MenuCatalogRow,
                    >::new(
                    ))));
                app.global::<riina_ui::PlayerMenuActions>()
                    .invoke_video_menu_changed();
                if let Err(error) = persist_recent_open_documents(
                    &clear_preferences_path,
                    &clear_recent_documents.borrow(),
                ) {
                    crate::diagnostics::report(
                        &app,
                        format!("Could not clear Open Recent: {error}"),
                    );
                }
                if let Err(error) = platform::clear_recent_documents() {
                    crate::diagnostics::report(
                        &app,
                        format!("Could not clear system recent documents: {error}"),
                    );
                }
            }
        });
}

fn install_url_open(
    app: &PlayerWindow,
    context: &Context<'_>,
    welcome_window: &WelcomeHandle,
    pending_welcome_open: &WelcomeRequests,
) {
    let commands = context.commands;
    let open_host = &context.open_host;
    let open_url = commands.clone();
    let weak = app.as_weak();
    let open_url_host = open_host.clone();
    let url_welcome = welcome_window.clone();
    let url_pending = pending_welcome_open.clone();
    app.global::<riina_ui::PlayerDialogState>()
        .on_open_url(move |value| {
            if let Some(app) = weak.upgrade() {
                let from_welcome = url_welcome
                    .as_ref()
                    .and_then(slint::Weak::upgrade)
                    .is_some_and(|welcome| welcome.window().is_visible());
                if from_welcome {
                    if let Err(error) = media_open::validate_url(value.as_str()) {
                        if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                            welcome.set_url_error(error.to_string().into());
                        }
                        return;
                    }
                    platform::set_native_video_host_visible(open_url_host.as_ref(), true);
                    if let Err(error) = platform::show_window(&app) {
                        app.set_status(error.to_string().into());
                        return;
                    }
                    if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                        welcome.set_url_error("".into());
                        // Continue releasing session resources if the window is closing.
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
}

fn install_help_and_debug(app: &PlayerWindow, latest_playback: &crate::PlaybackStore) {
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_open_help_link(move |destination| {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let Some(url) = help_menu_url(destination.as_str()) else {
                return;
            };
            if let Err(error) = crate::platform::open_http_url_in_browser(url) {
                crate::diagnostics::report(&app, format!("Could not open the Help link: {error}"));
            }
        });
    let weak = app.as_weak();
    let debug_playback = latest_playback.clone();
    app.global::<PlayerMenuActions>().on_dump_debug_info(move || {
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
            Err(error) => crate::diagnostics::report(&app, format!("Could not save debug information: {error}")),
        }
    });
}

fn install_welcome_recent(
    app: &PlayerWindow,
    context: &Context<'_>,
    welcome_window: &WelcomeHandle,
    pending_welcome_open: &WelcomeRequests,
) {
    let commands = context.commands;
    let open_host = &context.open_host;
    let recent_media = context.recent_media;
    let recent_player = commands.clone();
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
                platform::set_native_video_host_visible(recent_host.as_ref(), true);
                if let Err(error) = platform::show_window(&app) {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = recent_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    // Continue releasing session resources if the window is closing.
                    let _ = welcome.hide();
                }
                let request = PendingWelcomeOpen::Recent(
                    media,
                    app.global::<riina_ui::PlayerPreferences>()
                        .get_resume_last_position(),
                );
                if app.get_engine_ready() {
                    dispatch_welcome_open(&app, request, &recent_player);
                } else {
                    *recent_pending.borrow_mut() = Some(request);
                }
            } else {
                media_open::open_recent(
                    &app,
                    media,
                    app.global::<riina_ui::PlayerPreferences>()
                        .get_resume_last_position(),
                    recent_player.clone(),
                );
            }
        }
    });
}

fn install_media_opened(app: &PlayerWindow, context: &Context<'_>, welcome_window: &WelcomeHandle) {
    let recent_open_documents = context.recent_open_documents;
    let prefs_path = context.prefs_path;
    let opened_documents = recent_open_documents.clone();
    let opened_preferences_path = prefs_path.clone();
    let opened_app = app.as_weak();
    let opened_welcome = welcome_window.clone();
    app.on_media_opened(move |path, title| {
        if let Some(app) = opened_app.upgrade()
            && app
                .global::<riina_ui::PlayerPreferences>()
                .get_track_all_files_in_recent_open_menu()
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
            // Continue releasing session resources if the window is closing.
            let _ = welcome.hide();
        }
    });
}
