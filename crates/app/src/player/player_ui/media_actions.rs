//! File, URL, recent-document and welcome-window actions share one command sender.
pub(super) mod recent;

use super::{help_menu_url, recent_activity::RecentActivity, startup::SessionRecents};
use crate::{
    NativeVideoHost, PlayerWindow, WelcomeWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Command, sync::lock};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor},
};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

pub(super) struct Context<'a> {
    pub open_url: &'a Rc<crate::windows::open_url::ApplicationOpenUrl>,
    pub commands: &'a CommandSender,
    pub open_host: Option<slint::Weak<NativeVideoHost>>,
    pub welcome: Option<&'a WelcomeWindow>,
    pub recent: &'a SessionRecents,
    pub prefs_path: &'a PathBuf,
    pub latest_playback: &'a crate::PlaybackStore,
}

enum PendingWelcomeOpen {
    Stream(crate::windows::open_url::PendingStream),
    Recent(riina_core::RecentMedia, bool),
    Drop(media_open::WelcomeDrop, i32),
}

fn dispatch_welcome_open(
    app: &PlayerWindow,
    request: PendingWelcomeOpen,
    sender: &riina_mpv::CommandSender,
) {
    match request {
        PendingWelcomeOpen::Stream(stream) => stream.start(app, sender),
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
    install_resume_preference(
        app,
        context.commands,
        context.welcome,
        &context.recent.activity,
    );
    install_pause_on_open(app, context.commands);
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
    install_url_open(app, &context, &pending_welcome_open);
    install_help_links(app);
    install_welcome_recent(app, &context, &welcome_window, &pending_welcome_open);
    install_media_opened(app, &context, &welcome_window);
}

fn install_resume_preference(
    app: &PlayerWindow,
    commands: &CommandSender,
    welcome: Option<&WelcomeWindow>,
    recent: &Rc<RecentActivity>,
) {
    let resume_player = commands.clone();
    let resume_app = app.as_weak();
    app.on_resume_last_position_changed(move |enabled| {
        if let Some(app) = resume_app.upgrade() {
            if let Err(error) = resume_player.send(Command::SetResumeLastPosition(enabled)) {
                crate::diagnostics::report(&app, error);
            }
            app.invoke_recent_files_visibility_changed();
        }
    });
    let welcome = welcome.map(WelcomeWindow::as_weak);
    let weak = app.as_weak();
    let recent = recent.clone();
    app.on_recent_files_visibility_changed(move || {
        if let (Some(app), Some(welcome)) = (
            weak.upgrade(),
            welcome.as_ref().and_then(slint::Weak::upgrade),
        ) {
            recent.refresh_welcome(&app, &welcome);
        }
    });
}

fn install_pause_on_open(app: &PlayerWindow, commands: &CommandSender) {
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
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
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
        #[cfg(any(target_os = "macos", target_os = "windows"))]
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
    crate::playlist_export::install(app, open_host.clone(), latest_playback);
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
    let recent = &context.recent.activity;
    let remember = recent.clone();
    let weak = app.as_weak();
    app.on_remember_recent_path(move |path| {
        if let Some(app) = weak.upgrade() {
            let title = PathBuf::from(path.as_str()).file_name().map_or_else(
                || path.to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            remember.remember_document(&app, path.to_string(), title);
        }
    });
    let documents = recent.clone();
    let player = context.commands.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_open_recent_path(move |path| {
            let (Some(app), Some(media)) = (weak.upgrade(), documents.document(&path)) else {
                return;
            };
            documents.remember_document(&app, media.path.clone(), media.title.clone());
            media_open::open_recent(&app, media, false, player.clone());
        });
    let clear = recent.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_clear_open_recent(move || {
            if let Some(app) = weak.upgrade()
                && let Err(error) = clear.clear_documents()
            {
                crate::diagnostics::report(&app, format!("Could not clear Open Recent: {error}"));
            }
        });
}

fn install_url_open(app: &PlayerWindow, context: &Context<'_>, pending: &WelcomeRequests) {
    let sender = context.commands.clone();
    let open_host = context.open_host.clone();
    let pending = pending.clone();
    context
        .open_url
        .connect(app, context.welcome, move |app, stream| {
            if !app.window().is_visible() {
                platform::set_native_video_host_visible(open_host.as_ref(), true);
                if let Err(error) = platform::show_window(app) {
                    app.set_status(error.to_string().into());
                    return;
                }
            }
            submit_welcome_open(app, &sender, &pending, PendingWelcomeOpen::Stream(stream));
        });
}

fn install_help_links(app: &PlayerWindow) {
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
}

fn install_welcome_recent(
    app: &PlayerWindow,
    context: &Context<'_>,
    welcome_window: &WelcomeHandle,
    pending_welcome_open: &WelcomeRequests,
) {
    let commands = context.commands;
    let open_host = &context.open_host;
    let recent_player = commands.clone();
    let recent_host = open_host.clone();
    let recent_media_for_open = context.recent.activity.clone();
    let weak = app.as_weak();
    let recent_welcome = welcome_window.clone();
    let recent_pending = pending_welcome_open.clone();
    app.on_open_recent(move |index| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        if let Some(media) = recent_media_for_open.welcome_selection(&app, index) {
            let resume = index == -1;
            recent_media_for_open.remember_document(&app, media.path.clone(), media.title.clone());
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
                let request = PendingWelcomeOpen::Recent(media, resume);
                submit_welcome_open(&app, &recent_player, &recent_pending, request);
            } else {
                media_open::open_recent(&app, media, resume, recent_player.clone());
            }
        }
    });
}

fn install_media_opened(app: &PlayerWindow, context: &Context<'_>, welcome_window: &WelcomeHandle) {
    let recent = context.recent.activity.clone();
    let resume_revision = context.recent.resume_revision.clone();
    let opened_app = app.as_weak();
    let opened_welcome = welcome_window.clone();
    app.on_media_opened(move |path, title| {
        resume_revision.set(recent.revision());
        if let Some(app) = opened_app.upgrade()
            && app
                .global::<riina_ui::PlayerPreferences>()
                .get_track_all_files_in_recent_open_menu()
        {
            recent.remember_document(&app, path.to_string(), title.to_string());
        }
        if let Some(welcome) = opened_welcome.as_ref().and_then(slint::Weak::upgrade) {
            // Continue releasing session resources if the window is closing.
            let _ = welcome.hide();
        }
    });
}
