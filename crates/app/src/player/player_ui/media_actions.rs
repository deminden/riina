//! File, URL, recent-document and welcome-window actions share one command sender.
pub(super) mod recent;

use super::help_menu_url;
use crate::{
    MenuCatalogRow, NativeVideoHost, PlayerWindow, WelcomeWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use recent::{persist_recent_open_documents, remember_recent_open_document};
use riina_core::sync::lock;
use riina_core::{Command, RecentMedia};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor},
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
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
    Drop(media_open::WelcomeDrop, i32),
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
        PendingWelcomeOpen::Drop(drop, generation) => {
            if app.get_media_request_generation() == generation {
                drop.open(app, sender);
            }
        }
    }
}

type WelcomeHandle = Option<slint::Weak<WelcomeWindow>>;
type OpenHost = Option<slint::Weak<NativeVideoHost>>;
type WelcomeRequests = Arc<Mutex<Option<PendingWelcomeOpen>>>;

pub(super) fn install(app: &PlayerWindow, context: Context<'_>) {
    let welcome_window = context.welcome.map(ComponentHandle::as_weak);
    let pending_welcome_open = WelcomeRequests::default();
    install_renderer_ready(app, context.commands, &pending_welcome_open);
    install_external_file_actions(
        app,
        context.commands,
        &context.open_host,
        context.latest_playback,
    );
    install_file_open(app, context.commands, &context.open_host, &welcome_window);
    install_welcome_drop(app, &context, &pending_welcome_open);
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
        let request = lock(&ready_pending).take();
        if let Some(request) = request
            && let Some(app) = ready_app.upgrade()
        {
            dispatch_welcome_open(&app, request, &ready_player);
        }
    });
}

fn submit_welcome_open(
    app: &PlayerWindow,
    sender: &CommandSender,
    pending: &WelcomeRequests,
    request: PendingWelcomeOpen,
) {
    if app.get_engine_ready() {
        dispatch_welcome_open(app, request, sender);
    } else {
        *lock(pending) = Some(request);
    }
}

fn install_welcome_drop(app: &PlayerWindow, context: &Context<'_>, pending: &WelcomeRequests) {
    let Some(window) = context.welcome else {
        return;
    };
    let welcome = window.as_weak();
    let player = app.as_weak();
    let sender = context.commands.clone();
    let pending = pending.clone();
    let host = context.open_host.clone();
    let prefs_path = context.prefs_path.clone();
    let open = Rc::new(move |paths| {
        let (Some(welcome_window), Some(app)) = (welcome.upgrade(), player.upgrade()) else {
            return;
        };
        if !welcome_window.window().is_visible() {
            return;
        }
        let generation = media_open::invalidate(&app);
        welcome_window.set_drop_error("".into());
        let (welcome, player, sender, pending, host, prefs_path) = (
            welcome.clone(),
            player.clone(),
            sender.clone(),
            pending.clone(),
            host.clone(),
            prefs_path.clone(),
        );
        let started = crate::background::spawn_named("welcome-drop", move || {
            let result = crate::preference_load::read_valid(&prefs_path)
                .map_err(|error| error.to_string())
                .and_then(|prefs| {
                    media_open::WelcomeDrop::prepare(paths, &prefs.unwrap_or_default())
                });
            let _ = crate::background::to_ui(player, move |app| {
                let Some(welcome) = welcome.upgrade() else {
                    return;
                };
                if app.get_media_request_generation() != generation
                    || !welcome.window().is_visible()
                {
                    return;
                }
                match result {
                    Ok(drop) => {
                        platform::set_native_video_host_visible(host.as_ref(), true);
                        if let Err(error) = platform::show_window(&app) {
                            platform::set_native_video_host_visible(host.as_ref(), false);
                            welcome.set_drop_error(error.to_string().into());
                            return;
                        }
                        let _ = welcome.hide();
                        submit_welcome_open(
                            &app,
                            &sender,
                            &pending,
                            PendingWelcomeOpen::Drop(drop, generation),
                        );
                    }
                    Err(error) => welcome.set_drop_error(error.into()),
                }
            });
        });
        if let Err(error) = started {
            welcome_window.set_drop_error(error.to_string().into());
        }
    });
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let native_drop = {
        let open = open.clone();
        platform::desktop_drop::register(
            window,
            |_| {},
            move |_| {
                let open = open.clone();
                Some(Box::new(move |paths| open(paths)))
            },
        )
    };
    let (_, file_drop) = platform::batched_file_drops(|_| (), move |(), paths| open(paths));
    window.window().on_winit_window_event(move |window, event| {
        #[cfg(target_os = "linux")]
        let _ = &native_drop;
        #[cfg(target_os = "windows")]
        native_drop.attach(window);
        file_drop(window, event);
        EventResult::Propagate
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
                media_open::invalidate(&app);
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
                    media_open::invalidate(&app);
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
                    submit_welcome_open(&app, &open_url, &url_pending, request);
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
                media_open::invalidate(&app);
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
                submit_welcome_open(&app, &recent_player, &recent_pending, request);
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
