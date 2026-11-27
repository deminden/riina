//! Filesystem preparation never runs inside a rendering callback or GUI event.
mod desktop_drop;
mod routing;

use super::{player_ui::ObserverId, playlist_edits, surface::PlayerDisplay};
use crate::{PlayerWindow, file_selection};
pub(crate) use desktop_drop::dropped;
use riina_core::Command;
use riina_mpv::CommandSender;
pub(in crate::player) use routing::PlayerSelection;
pub(crate) use routing::duplicate_player;
use routing::prepare_new_players;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{ffi::OsString, sync::atomic::Ordering};

pub(crate) fn apply_manual_open_repeat(
    app: &PlayerWindow,
    sender: &CommandSender,
) -> Result<(), String> {
    if let Some(command) = manual_open_repeat_command(
        app.global::<riina_ui::PlayerPreferences>()
            .get_auto_repeat(),
        app.global::<riina_ui::PlayerPreferences>()
            .get_default_repeat_mode_index(),
    )? {
        sender.send(command).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn manual_open_repeat_command(enabled: bool, mode_index: i32) -> Result<Option<Command>, String> {
    if !enabled {
        return Ok(None);
    }
    let mode = riina_core::DefaultRepeatMode::from_index(mode_index)
        .ok_or_else(|| "The saved default repeat mode is invalid".to_owned())?;
    Ok(Some(Command::SetLoopMode(mode.loop_mode())))
}

pub(super) fn playlist_in_new_player(app: &PlayerWindow, paths: Vec<String>) {
    new_player_selection(app, paths, prepare_explicit_player);
}

pub(crate) fn history_playlist(
    app: &PlayerWindow,
    display: &PlayerDisplay,
    paths: Vec<String>,
    sender: CommandSender,
) -> Result<(), String> {
    display.reveal(app).map_err(|error| error.to_string())?;
    display.focus(app);
    selection_at(
        app,
        paths,
        None,
        sender,
        SelectionSource::ExplicitPlaylist,
        ObserverId::for_window(app),
    );
    Ok(())
}

pub(crate) fn history_entry(
    app: &PlayerWindow,
    display: &PlayerDisplay,
    path: &str,
    sender: CommandSender,
) -> Result<(), String> {
    // The history row is a cached display. Load normally so mpv applies its
    // current saved position and resume preference for the destination player.
    if open_single_in_separate_player(app, path, None)? {
        return Ok(());
    }
    display.reveal(app).map_err(|error| error.to_string())?;
    display.focus(app);
    let observer = ObserverId::for_window(app);
    if let Ok(address) = validate_url(path)
        && let Some(opening) = observer.and_then(ObserverId::open_url)
    {
        return opening.open_initial_stream(app, &sender, address, None);
    }
    selection_at(
        app,
        vec![path.to_owned()],
        None,
        sender,
        SelectionSource::Replace,
        observer,
    );
    Ok(())
}

fn new_player_selection(
    app: &PlayerWindow,
    paths: Vec<String>,
    prepare: fn(Vec<String>) -> Result<Vec<PlayerSelection>, String>,
) {
    if paths.is_empty() {
        return;
    }
    if paths
        .iter()
        .any(|path| path.is_empty() || path.contains('\0'))
    {
        app.set_status("The media selection contains an invalid reference".into());
        return;
    }
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_activity();
    app.set_status("Preparing media for a new player…".into());
    let weak = app.as_weak();
    let failure = weak.clone();
    let started = crate::background::spawn_named("new-player-media-discovery", move || {
        let result = prepare(paths);
        // The result has no recipient once its window or event loop closes.
        let _ = crate::background::to_ui(weak, move |app| {
            if let Err(error) = result.and_then(|players| {
                if players.is_empty() {
                    return Err("No playable files were found".into());
                }
                open_new_players(&app, players)
            }) {
                app.set_status(error.into());
            }
        });
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.set_status(error.to_string().into());
    }
}

fn prepare_file_dialog_players(paths: Vec<String>) -> Result<Vec<PlayerSelection>, String> {
    prepare_new_players(paths, &preferences_for_open()?, || false)
}

fn prepare_explicit_player(paths: Vec<String>) -> Result<Vec<PlayerSelection>, String> {
    let paths = file_selection::prepare(paths, true)?;
    Ok((!paths.is_empty())
        .then_some(PlayerSelection {
            paths,
            duplicate_reference: None,
            explicit_playlist: true,
        })
        .into_iter()
        .collect())
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
pub(super) fn prepare_application_documents(
    paths: Vec<String>,
    preferences: &riina_core::Preferences,
    alternative: bool,
) -> Result<Vec<PlayerSelection>, String> {
    let players = if preferences.allow_multiple_windows != alternative {
        prepare_new_players(paths, preferences, || false)?
    } else {
        let paths = file_selection::prepare(paths, true)?;
        (!paths.is_empty())
            .then_some(PlayerSelection {
                paths,
                duplicate_reference: None,
                explicit_playlist: false,
            })
            .into_iter()
            .collect()
    };
    if players.is_empty() {
        Err("No playable files were found".into())
    } else {
        Ok(players)
    }
}

fn open_new_players(
    app: &PlayerWindow,
    players: impl IntoIterator<Item = PlayerSelection>,
) -> Result<(), String> {
    for player in players {
        open_player(
            app,
            &player.paths,
            player.duplicate_reference.as_deref(),
            None,
            player.explicit_playlist,
        )?;
    }
    Ok(())
}

fn open_player(
    app: &PlayerWindow,
    paths: &[String],
    duplicate_reference: Option<&str>,
    resume_position: Option<f64>,
    explicit_playlist: bool,
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    if !app.get_player_manager_ready() {
        return Err("The player window is unavailable".into());
    }
    app.set_status("Opening in a new player…".into());
    app.invoke_open_player_requested(
        ModelRc::new(VecModel::from(
            paths
                .iter()
                .map(|path| SharedString::from(path.as_str()))
                .collect::<Vec<_>>(),
        )),
        duplicate_reference.unwrap_or_default().into(),
        resume_position.map_or(-1.0, |position| position as f32),
        explicit_playlist,
    );
    Ok(())
}

pub(crate) fn same_media(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    if left.contains("://") || right.contains("://") {
        return false;
    }
    std::path::Path::new(left)
        .canonicalize()
        .ok()
        .zip(std::path::Path::new(right).canonicalize().ok())
        .is_some_and(|(left, right)| left == right)
}

pub(crate) fn current_media(app: &PlayerWindow) -> Option<String> {
    app.global::<riina_ui::PlayerCatalogState>()
        .get_playlist()
        .iter()
        .find(|row| row.playing)
        .map(|row| row.path.to_string())
}

pub(crate) fn has_playlist(app: &PlayerWindow) -> bool {
    app.global::<riina_ui::PlayerCatalogState>()
        .get_playlist()
        .row_count()
        > 0
}

fn preferences_for_open() -> Result<riina_core::Preferences, String> {
    let preferences_path =
        crate::platform::preferences_path().map_err(|error| error.to_string())?;
    crate::preference_load::read_valid(&preferences_path)
        .map(Option::unwrap_or_default)
        .map_err(|error| error.to_string())
}

fn open_single_in_separate_player(
    app: &PlayerWindow,
    path: &str,
    resume_position: Option<f64>,
) -> Result<bool, String> {
    if !has_playlist(app) || !preferences_for_open()?.allow_multiple_windows {
        return Ok(false);
    }
    open_player(app, &[path.to_owned()], Some(path), resume_position, false)?;
    Ok(true)
}

pub(crate) fn stream_in_new_player(app: &PlayerWindow, alternative: bool) -> Result<bool, String> {
    Ok(has_playlist(app) && (preferences_for_open()?.allow_multiple_windows != alternative))
}

pub(crate) struct WelcomeDrop {
    command: Command,
    new_players: Vec<PlayerSelection>,
}

impl WelcomeDrop {
    pub(crate) fn prepare(
        paths: Vec<OsString>,
        preferences: &riina_core::Preferences,
    ) -> Result<Self, String> {
        let paths = paths
            .into_iter()
            .map(OsString::into_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "A media path contains invalid Unicode".to_owned())?;
        if preferences.allow_multiple_windows {
            let mut players = prepare_new_players(paths, preferences, || false)?.into_iter();
            let first = players
                .next()
                .ok_or("Drop a playable file or folder to open it.")?;
            return Ok(Self {
                command: Command::OpenFiles(first.paths),
                new_players: players.collect(),
            });
        }
        let paths = file_selection::prepare(paths, true)?;
        if paths.is_empty() {
            return Err("Drop a playable file or folder to open it.".into());
        }
        Ok(Self {
            command: Command::OpenFiles(paths),
            new_players: Vec::new(),
        })
    }

    pub(crate) fn open(self, app: &PlayerWindow, sender: &CommandSender) {
        app.set_status("Opening…".into());
        let result = (|| {
            open_prepared_command(
                app,
                sender,
                self.command,
                ObserverId::for_window(app),
                app.get_media_request_generation(),
            )?;
            open_new_players(app, self.new_players)?;
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            app.set_status(error.into());
        }
    }
}

pub(super) fn startup_at(
    app: &PlayerWindow,
    media: crate::player_ui::InitialMedia,
    sender: CommandSender,
    observer: ObserverId,
) {
    let grouped_stream = media.managed.is_none().then_some(observer);
    startup_selection(
        app,
        media.files,
        media.resume_position,
        sender,
        if media.managed.is_some() {
            SelectionSource::Plugin
        } else if media.launch.shuffle {
            SelectionSource::StartupShuffled
        } else if media.explicit_playlist {
            SelectionSource::ExplicitPlaylist
        } else {
            SelectionSource::Replace
        },
        grouped_stream,
    );
}

pub(crate) fn startup_stdin(app: &PlayerWindow, sender: &CommandSender) {
    invalidate(app);
    app.set_status("Opening…".into());
    if let Err(error) = sender.send(Command::Load("-".into())) {
        crate::diagnostics::report(app, error);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectionSource {
    Replace,
    StartupShuffled,
    ExplicitPlaylist,
    Plugin,
}

fn startup_selection(
    app: &PlayerWindow,
    paths: Vec<OsString>,
    resume_position: Option<f64>,
    sender: CommandSender,
    source: SelectionSource,
    grouped_stream: Option<ObserverId>,
) {
    let paths = paths
        .into_iter()
        .map(OsString::into_string)
        .collect::<Result<Vec<_>, _>>();
    match paths {
        Ok(paths) => selection_at(app, paths, resume_position, sender, source, grouped_stream),
        Err(_) => app.set_status("A media path contains invalid Unicode".into()),
    }
}

pub fn resume(app: &PlayerWindow, media: riina_core::RecentMedia, sender: CommandSender) {
    match open_single_in_separate_player(app, &media.path, Some(media.position)) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    }
    invalidate(app);
    app.set_status("Resuming…".into());
    if let Err(error) = sender.send(Command::LoadAtPosition {
        path: media.path,
        position: media.position,
    }) {
        crate::diagnostics::report(app, error);
    } else if let Err(error) = apply_manual_open_repeat(app, &sender) {
        app.set_status(error.into());
    }
}

pub fn open_recent(
    app: &PlayerWindow,
    media: riina_core::RecentMedia,
    resume_last_position: bool,
    sender: CommandSender,
) {
    if file_selection::is_shortcut_reference(&media.path) {
        file_dialog_selection(app, vec![media.path], false, sender);
        return;
    }
    if resume_last_position {
        resume(app, media, sender);
        return;
    }
    match open_single_in_separate_player(app, &media.path, None) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    }
    invalidate(app);
    app.set_status("Opening…".into());
    if let Err(error) = sender.send(Command::Load(media.path)) {
        crate::diagnostics::report(app, error);
    } else if let Err(error) = apply_manual_open_repeat(app, &sender) {
        app.set_status(error.into());
    }
}

pub fn validate_url(value: &str) -> Result<&str, riina_core::media_reference::NetworkUrlError> {
    riina_core::media_reference::validate_network_url(value)
}

pub fn invalidate(app: &PlayerWindow) -> i32 {
    if let Some(observer) = ObserverId::for_window(app) {
        observer.cancel_discovery();
        observer.supersede_media();
    }
    playlist_edits::retire_media(app);
    let generation = app.get_media_request_generation().wrapping_add(1);
    app.set_media_request_generation(generation);
    generation
}

pub(crate) fn track_stream_attempt(app: &PlayerWindow, attempt: u64) {
    if let Some(observer) = ObserverId::for_window(app) {
        observer.track_stream_attempt(attempt);
    }
}

pub(super) fn stop_command(app: &PlayerWindow) -> Command {
    let request = invalidate(app).cast_unsigned();
    if let Some(observer) = ObserverId::for_window(app) {
        observer.arm_idle_after_stop(request);
    }
    Command::Stop(Some(request))
}

pub(crate) fn file_dialog_selection(
    app: &PlayerWindow,
    paths: Vec<String>,
    alternative: bool,
    sender: CommandSender,
) {
    if app
        .global::<riina_ui::PlayerPreferences>()
        .get_open_in_new_player()
        != alternative
    {
        // Leave the caller's playback and pending work alone. The manager
        // checks duplicates and reserves any idle player after preparation.
        new_player_selection(app, paths, prepare_file_dialog_players);
        return;
    }
    selection_at(
        app,
        paths,
        None,
        sender,
        SelectionSource::Replace,
        ObserverId::for_window(app),
    );
}

fn selection_at(
    app: &PlayerWindow,
    paths: Vec<String>,
    resume_position: Option<f64>,
    sender: CommandSender,
    source: SelectionSource,
    grouped_stream: Option<ObserverId>,
) {
    if paths.is_empty() {
        return;
    }
    let generation = invalidate(app);
    playlist_edits::hold_for_open(app);
    let discovery = ObserverId::for_window(app)
        .map(ObserverId::begin_discovery)
        .unwrap_or_default();
    let weak = app.as_weak();
    let commands = sender.clone();
    let (ready, result) = tokio::sync::oneshot::channel();
    app.set_status("Opening…".into());
    if let Err(error) = slint::spawn_local(async move {
        let prepared = result.await;
        let Some(app) = weak.upgrade() else { return };
        // A newer accepted selection or Stop owns the player now.
        if app.get_media_request_generation() != generation {
            return;
        }
        let result = prepared
            .unwrap_or_else(|_| Err("Could not prepare media".into()))
            .and_then(|command: Command| {
                open_prepared_command(&app, &commands, command, grouped_stream, generation)
            });
        if let Err(error) = result {
            app.set_status(error.into());
        }
        if app.get_media_request_generation() == generation {
            playlist_edits::release_after_open(&app, &commands);
        }
    }) {
        app.set_status(error.to_string().into());
        playlist_edits::release_after_open(app, &sender);
        return;
    }
    if let Err(error) = crate::background::spawn_named("media-discovery", move || {
        let cancelled = || discovery.load(Ordering::Relaxed);
        let result = prepare_selection(paths, resume_position, source, cancelled);
        let _ = ready.send(result);
    }) {
        // Dropping the sender wakes the UI task and releases queued additions.
        crate::diagnostics::warn!("Could not start media discovery: {error}");
    }
}

fn prepare_selection(
    paths: Vec<String>,
    resume_position: Option<f64>,
    source: SelectionSource,
    cancelled: impl Fn() -> bool,
) -> Result<Command, String> {
    let paths = if source == SelectionSource::Plugin {
        file_selection::prepare_plugin_sources(paths, cancelled)?
    } else {
        file_selection::prepare_cancellable(paths, true, cancelled)?
    };
    if paths.is_empty() {
        return Err("No playable files were found".into());
    }
    let command = if source == SelectionSource::StartupShuffled {
        Command::OpenShuffledPlaylist(paths)
    } else if source == SelectionSource::ExplicitPlaylist {
        Command::OpenPlaylist(paths)
    } else if let Some(position) =
        resume_position.filter(|value| value.is_finite() && *value >= 0.0)
        && let [path] = paths.as_slice()
    {
        Command::LoadAtPosition {
            path: path.clone(),
            position,
        }
    } else {
        Command::OpenFiles(paths)
    };
    Ok(command)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn prepared_documents(
    app: &PlayerWindow,
    paths: Vec<String>,
    sender: &CommandSender,
) -> Result<(), String> {
    let generation = invalidate(app);
    app.set_status("Opening…".into());
    open_prepared_command(
        app,
        sender,
        Command::OpenFiles(paths),
        ObserverId::for_window(app),
        generation,
    )
}

fn open_prepared_command(
    app: &PlayerWindow,
    sender: &CommandSender,
    command: Command,
    grouped_stream: Option<ObserverId>,
    generation: i32,
) -> Result<(), String> {
    if let Some((address, following, explicit)) = grouped_stream_paths(&command)
        && let Some(opening) = grouped_stream.and_then(ObserverId::open_url)
    {
        opening.open_initial_playlist(app, sender, address, following, explicit, generation)
    } else {
        sender.send(command)?;
        apply_manual_open_repeat(app, sender)
    }
}

fn grouped_stream_paths(command: &Command) -> Option<(&str, &[String], bool)> {
    let paths = match command {
        Command::OpenFiles(paths) | Command::OpenPlaylist(paths) => paths,
        _ => return None,
    };
    let (address, following) = paths.split_first()?;
    let stream = validate_url(address).ok()?;
    Some((
        stream,
        following,
        matches!(command, Command::OpenPlaylist(_)),
    ))
}

#[cfg(test)]
mod tests {
    use super::{manual_open_repeat_command, validate_url};
    use riina_core::{Command, LoopMode};

    #[test]
    fn new_player_requests_require_a_live_manager_and_keep_the_media_intent()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::open_player;
        use slint::Model;
        use std::{cell::RefCell, rc::Rc};

        i_slint_backend_testing::init_no_event_loop();
        let app = crate::PlayerWindow::new()?;
        let requests = Rc::new(RefCell::new(Vec::new()));
        let delivered = requests.clone();
        app.on_open_player_requested(move |paths, reference, position, explicit_playlist| {
            delivered.borrow_mut().push((
                paths
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>(),
                reference.to_string(),
                position,
                explicit_playlist,
            ));
        });
        let paths = ["First Clip.mkv".to_owned(), "字幕.mkv".to_owned()];

        assert!(open_player(&app, &paths[..1], Some(&paths[0]), Some(3.5), false).is_err());
        assert!(requests.borrow().is_empty());

        app.set_player_manager_ready(true);
        open_player(&app, &paths[..1], Some(&paths[0]), Some(3.5), false)?;
        open_player(&app, &paths, None, None, true)?;

        app.set_player_manager_ready(false);
        assert!(open_player(&app, &paths, None, None, true).is_err());
        assert_eq!(
            *requests.borrow(),
            [
                (vec![paths[0].clone()], paths[0].clone(), 3.5, false),
                (paths.to_vec(), String::new(), -1.0, true),
            ]
        );
        Ok(())
    }

    #[test]
    fn windowless_document_opens_follow_the_saved_window_and_grouping_preferences() {
        let files = || vec!["clip10.avi".into(), "clip2.avi".into()];
        let paths = |preferences: &riina_core::Preferences, alternative| {
            super::prepare_application_documents(files(), preferences, alternative)
                .unwrap()
                .into_iter()
                .map(|media| media.paths)
                .collect::<Vec<_>>()
        };
        let mut preferences = riina_core::Preferences::default();
        let separate_plans =
            super::prepare_application_documents(files(), &preferences, false).unwrap();
        assert_eq!(
            separate_plans[0].duplicate_reference.as_deref(),
            Some("clip10.avi")
        );
        let current_plans =
            super::prepare_application_documents(files(), &preferences, true).unwrap();
        assert_eq!(current_plans[0].duplicate_reference, None);

        let grouped = [vec!["clip2.avi", "clip10.avi"]];
        let separate = [vec!["clip10.avi"], vec!["clip2.avi"]];
        assert_eq!(paths(&preferences, false), separate);
        assert_eq!(paths(&preferences, true), grouped);
        preferences.group_simultaneous_opens_in_playlist = true;
        assert_eq!(paths(&preferences, false), grouped);
        preferences.group_simultaneous_opens_in_playlist = false;
        preferences.allow_multiple_windows = false;
        assert_eq!(paths(&preferences, false), grouped);
        assert_eq!(paths(&preferences, true), separate);
        assert!(
            super::prepare_application_documents(vec!["captions.srt".into()], &preferences, false)
                .is_err()
        );
    }

    #[test]
    fn grouped_startup_tracks_only_an_open_with_a_network_lead() {
        use super::{SelectionSource, grouped_stream_paths, prepare_selection};

        let address = "https://media.example/opening.avi";
        let second = "https://media.example/closing.avi";
        let prepared = prepare_selection(
            vec![
                address.into(),
                "part10.avi".into(),
                "part2.avi".into(),
                second.into(),
            ],
            None,
            SelectionSource::ExplicitPlaylist,
            || false,
        )
        .unwrap();
        let (first, following, explicit) = grouped_stream_paths(&prepared).unwrap();
        assert_eq!(first, address);
        assert_eq!(following, ["part2.avi", "part10.avi", second]);
        assert!(explicit);
        let local_first = vec!["local.avi".into(), address.into()];
        assert!(grouped_stream_paths(&Command::OpenFiles(local_first.clone())).is_none());
        assert!(grouped_stream_paths(&Command::OpenShuffledPlaylist(local_first)).is_none());
        assert!(grouped_stream_paths(&Command::AppendFiles(vec![address.into()])).is_none());
    }

    #[test]
    fn history_actions_keep_the_selection_in_one_explicit_playlist() {
        use super::{SelectionSource, prepare_explicit_player, prepare_selection};

        let paths = vec!["clip10.mkv".into(), "clip2.mkv".into()];
        let expected = ["clip2.mkv", "clip10.mkv"];
        let current = prepare_selection(
            paths.clone(),
            None,
            SelectionSource::ExplicitPlaylist,
            || false,
        );
        assert!(matches!(current, Ok(Command::OpenPlaylist(paths)) if paths == expected));

        let new = prepare_explicit_player(paths).unwrap();
        let [player] = new.as_slice() else {
            panic!("History opens one player")
        };
        assert_eq!(player.paths, expected);
        assert!(player.explicit_playlist);
        assert!(player.duplicate_reference.is_none());
    }

    #[test]
    fn welcome_drops_route_plain_media_through_the_window_preferences() {
        use super::WelcomeDrop;
        use riina_core::Preferences;

        let paths = || vec!["a.mkv".into(), "b.mkv".into()];
        let mut prefs = Preferences {
            allow_multiple_windows: true,
            group_simultaneous_opens_in_playlist: false,
            ..Preferences::default()
        };
        let split = WelcomeDrop::prepare(paths(), &prefs).unwrap();
        assert_eq!(split.command, Command::OpenFiles(vec!["a.mkv".into()]));
        assert_eq!(split.new_players.len(), 1);
        assert_eq!(split.new_players[0].paths, ["b.mkv"]);

        prefs.group_simultaneous_opens_in_playlist = true;
        let grouped = WelcomeDrop::prepare(paths(), &prefs).unwrap();
        assert_eq!(
            grouped.command,
            Command::OpenFiles(vec!["a.mkv".into(), "b.mkv".into()])
        );
        assert!(grouped.new_players.is_empty());

        prefs.allow_multiple_windows = false;
        prefs.group_simultaneous_opens_in_playlist = false;
        let single_player = WelcomeDrop::prepare(paths(), &prefs).unwrap();
        assert_eq!(single_player.command, grouped.command);
        assert!(single_player.new_players.is_empty());
    }

    #[test]
    fn welcome_drops_require_playable_media_even_when_subtitles_are_present() {
        use super::WelcomeDrop;
        use riina_core::Preferences;

        let prefs = Preferences {
            allow_multiple_windows: true,
            ..Preferences::default()
        };
        assert!(WelcomeDrop::prepare(vec!["subtitles.srt".into()], &prefs).is_err());
        let mixed = WelcomeDrop::prepare(
            vec!["a.mkv".into(), "b.mkv".into(), "subtitles.srt".into()],
            &prefs,
        )
        .unwrap();
        assert_eq!(mixed.command, Command::OpenFiles(vec!["a.mkv".into()]));
        assert_eq!(mixed.new_players.len(), 1);
        assert_eq!(mixed.new_players[0].paths, ["b.mkv"]);
    }

    #[test]
    fn manual_open_repeat_is_opt_in() {
        assert_eq!(manual_open_repeat_command(false, 0), Ok(None));
    }

    #[test]
    fn manual_open_repeat_maps_the_selected_loop_mode() {
        assert_eq!(
            manual_open_repeat_command(true, 0),
            Ok(Some(Command::SetLoopMode(LoopMode::Playlist)))
        );
        assert_eq!(
            manual_open_repeat_command(true, 1),
            Ok(Some(Command::SetLoopMode(LoopMode::File)))
        );
    }

    #[test]
    fn invalid_manual_repeat_mode_is_rejected_only_when_enabled() {
        assert!(manual_open_repeat_command(false, 99).is_ok());
        assert!(manual_open_repeat_command(true, 99).is_err());
    }

    #[test]
    fn accepts_only_absolute_network_media_urls() {
        assert_eq!(
            validate_url(" https://example.test/movie.mkv ").unwrap(),
            "https://example.test/movie.mkv"
        );
        assert_eq!(
            validate_url("RTSP://camera.test/live").unwrap(),
            "RTSP://camera.test/live"
        );
        for rejected in [
            "",
            "movie.mkv",
            "file:///movie.mkv",
            "https://",
            "https://bad host/x",
            "https://x\0y",
        ] {
            assert!(validate_url(rejected).is_err(), "accepted {rejected:?}");
        }
    }
}
