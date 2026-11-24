//! Application-scoped Inspector for the most recently focused player.
mod presentation;
mod properties;
mod sample;
mod track;
mod watch;

use crate::{PlaybackStore, PlayerWindow, platform, ui_models::set_rows_if_changed};
use riina_core::{Phase, Playback, Track, sync::lock};
use riina_mpv::CommandSender;
use riina_ui::{MediaInspectorWindow, PlayerMenuActions};
use sample::{Request, Sampler, Subject};
use slint::{CloseRequestResponse, ComponentHandle, Timer, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::Arc,
    time::Instant,
};

#[derive(Default)]
pub(crate) struct ApplicationInspector {
    window: RefCell<Option<MediaInspectorWindow>>,
    players: RefCell<Vec<Weak<Target>>>,
    sampler: RefCell<Sampler>,
    catalog: RefCell<Catalog>,
    watches: RefCell<Vec<String>>,
    watches_revision: Cell<u64>,
    timer: Rc<Timer>,
}

struct Target {
    app: slint::Weak<PlayerWindow>,
    playback: PlaybackStore,
    commands: CommandSender,
    preferences: PathBuf,
}

#[derive(Default)]
struct Catalog {
    source: Option<(usize, u64)>,
    tracks: Arc<Vec<Track>>,
    order: Vec<usize>,
    revision: u64,
}

#[derive(Clone)]
struct Frame {
    target: Rc<Target>,
    playback: Arc<Playback>,
    subject: Subject,
}

impl Frame {
    fn selected_track(&self) -> Option<(usize, &Track)> {
        let index = self.subject.track?;
        self.playback.tracks.get(index).map(|track| (index, track))
    }
}

impl ApplicationInspector {
    pub(crate) fn connect(
        self: &Rc<Self>,
        app: &PlayerWindow,
        playback: PlaybackStore,
        commands: CommandSender,
        preferences: &Path,
    ) {
        let target = Rc::new(Target {
            app: app.as_weak(),
            playback,
            commands,
            preferences: preferences.to_owned(),
        });
        let owner = self.clone();
        let activated = target.clone();
        app.on_inspector_player_activated(move || {
            owner.activate(&activated);
            if let Some(window) = owner.visible_window() {
                owner.refresh(&window);
            }
        });
        let owner = self.clone();
        app.global::<PlayerMenuActions>()
            .on_show_inspector(move || {
                if let Some(app) = target.app.upgrade() {
                    owner.activate(&target);
                    if let Err(error) = owner.show(&app, &target.preferences) {
                        crate::diagnostics::report(
                            &app,
                            format!("Could not open Inspector: {error}"),
                        );
                    }
                }
            });
    }

    fn activate(&self, target: &Rc<Target>) {
        let mut players = self.players.borrow_mut();
        players.retain(|source| source.strong_count() > 0 && source.as_ptr() != Rc::as_ptr(target));
        players.push(Rc::downgrade(target));
    }

    fn visible_window(&self) -> Option<MediaInspectorWindow> {
        self.window
            .borrow()
            .as_ref()
            .filter(|window| window.window().is_visible())
            .map(ComponentHandle::clone_strong)
    }

    fn show(
        self: &Rc<Self>,
        app: &PlayerWindow,
        preferences: &Path,
    ) -> Result<(), slint::PlatformError> {
        let existing = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        let window = if let Some(window) = existing {
            window
        } else {
            let window = MediaInspectorWindow::new()?;
            crate::appearance_preferences::install_system_typography(&window);
            platform::center_auxiliary_window(&window, app);
            self.bind(&window, preferences);
            self.window.borrow_mut().replace(window.clone_strong());
            window
        };
        if !window.window().is_visible() {
            self.reload_watches(&window, preferences);
        }
        platform::show_window(&window)?;
        window.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        self.refresh(&window);
        if !self.timer.running() {
            let owner = Rc::downgrade(self);
            super::visible_refresh::run(&window, &self.timer, sample::INTERVAL, move |window| {
                if let Some(owner) = owner.upgrade() {
                    owner.refresh(window);
                }
            });
        }
        Ok(())
    }

    fn bind(self: &Rc<Self>, window: &MediaInspectorWindow, preferences: &Path) {
        let owner = Rc::downgrade(self);
        let weak_window = window.as_weak();
        window.window().on_close_requested(move || {
            if let Some(owner) = owner.upgrade() {
                owner.timer.stop();
                owner.sampler.borrow_mut().select(None);
            }
            if let Some(window) = weak_window.upgrade() {
                window.set_watch_prompt_open(false);
                window.set_watch_name("".into());
            }
            CloseRequestResponse::HideWindow
        });

        let owner = Rc::downgrade(self);
        let weak_window = window.as_weak();
        let refresh = move || {
            if let (Some(owner), Some(window)) = (owner.upgrade(), weak_window.upgrade()) {
                owner.refresh(&window);
            }
        };
        window.on_tab_selected(refresh.clone());
        window.on_track_selected(refresh);
        self.bind_watches(window, preferences);

        let weak_window = window.as_weak();
        window.on_open_link(move |url| {
            if let Err(error) = platform::open_http_url_in_browser(url.as_str())
                && let Some(window) = weak_window.upgrade()
            {
                window.set_message(error.to_string().into());
            }
        });
    }

    fn bind_watches(self: &Rc<Self>, window: &MediaInspectorWindow, preferences: &Path) {
        let owner = Rc::downgrade(self);
        let weak_window = window.as_weak();
        let path = preferences.to_owned();
        window.on_watch_added(move |name| {
            if let (Some(owner), Some(window)) = (owner.upgrade(), weak_window.upgrade()) {
                match watch::add(&path, name.as_str()) {
                    Ok(watches) => {
                        window.set_watch_name("".into());
                        window.set_watch_prompt_open(false);
                        window.set_selected_watch(
                            i32::try_from(watches.len().saturating_sub(1)).unwrap_or(-1),
                        );
                        owner.set_watches(&window, watches);
                        owner.refresh(&window);
                    }
                    Err(error) => window.set_message(error.into()),
                }
            }
        });

        let owner = Rc::downgrade(self);
        let weak_window = window.as_weak();
        let path = preferences.to_owned();
        window.on_watch_removed(move |index| {
            if let (Some(owner), Some(window), Ok(index)) = (
                owner.upgrade(),
                weak_window.upgrade(),
                usize::try_from(index),
            ) {
                let updated = watch::remove(&path, &owner.watches.borrow(), &[index]);
                match updated {
                    Ok(watches) => {
                        window.set_selected_watch(-1);
                        owner.set_watches(&window, watches);
                    }
                    Err(error) => {
                        owner.reload_watches(&window, &path);
                        window.set_message(error.into());
                    }
                }
                owner.refresh(&window);
            }
        });
    }

    fn reload_watches(&self, window: &MediaInspectorWindow, path: &Path) {
        match watch::load(path) {
            Ok(watches) => self.set_watches(window, watches),
            Err(error) => window.set_message(error.into()),
        }
    }

    fn set_watches(&self, window: &MediaInspectorWindow, watches: Vec<String>) {
        if *self.watches.borrow() != watches {
            self.watches.replace(watches);
            self.watches_revision
                .set(self.watches_revision.get().wrapping_add(1));
        }
        window.set_message("".into());
    }

    fn refresh(self: &Rc<Self>, window: &MediaInspectorWindow) {
        let Some(frame) = self.synchronize(window) else {
            return;
        };
        let Some(request) = self.sampler.borrow_mut().request(Instant::now()) else {
            return;
        };
        let properties = properties::for_tab(
            frame.subject.tab,
            frame.subject.track,
            &self.watches.borrow(),
        );
        let commands = frame.target.commands.clone();
        let worker = request.clone();
        let (ready, response) = tokio::sync::oneshot::channel();
        if let Err(error) = crate::background::spawn_named("riina-inspector", move || {
            let _ = ready.send(properties::read(&commands, properties, &worker));
        }) {
            self.sampler.borrow_mut().complete(&request);
            window.set_message(error.to_string().into());
            return;
        }

        let owner = Rc::downgrade(self);
        let task = request.clone();
        if let Err(error) = slint::spawn_local(async move {
            let values = response.await.ok();
            if let Some(owner) = owner.upgrade() {
                owner.complete(&task, values.as_ref());
            }
        }) {
            self.sampler.borrow_mut().select(None);
            self.sampler.borrow_mut().complete(&request);
            window.set_message(error.to_string().into());
        }
    }

    fn complete(self: &Rc<Self>, request: &Request, values: Option<&properties::Values>) {
        let window = self.visible_window();
        let frame = window.as_ref().and_then(|window| self.synchronize(window));
        if window.is_none() {
            self.sampler.borrow_mut().select(None);
        }
        let completed = self.sampler.borrow_mut().complete(request);
        if let Some(window) = &window {
            match (completed, frame, values) {
                (Some(subject), Some(frame), Some(values)) if subject == frame.subject => {
                    presentation::render(
                        window,
                        &frame.playback,
                        frame.selected_track(),
                        &self.watches.borrow(),
                        values,
                    );
                }
                (None, Some(_), _) => self.refresh(window),
                _ => {}
            }
        }
    }

    fn synchronize(&self, window: &MediaInspectorWindow) -> Option<Frame> {
        let target = {
            let mut players = self.players.borrow_mut();
            players.retain(|target| target.strong_count() > 0);
            players.last().and_then(Weak::upgrade)
        };
        let frame = target.map(|target| {
            let playback = lock(&target.playback).clone();
            let source = (Rc::as_ptr(&target) as usize, playback.generation);
            let mut catalog = self.catalog.borrow_mut();
            let ready = matches!(
                playback.phase,
                Phase::Playing | Phase::Paused | Phase::Seeking | Phase::Buffering
            );
            let track = catalog.update(window, source, &playback.tracks, ready);
            let subject = Subject {
                target: source.0,
                generation: source.1,
                tab: window.get_selected_tab(),
                track,
                catalog: catalog.revision,
                watches: self.watches_revision.get(),
                ready,
            };
            Frame {
                target,
                playback,
                subject,
            }
        });
        let changed = self
            .sampler
            .borrow_mut()
            .select(frame.as_ref().map(|frame| frame.subject));
        if changed || frame.is_none() {
            let empty = Playback::default();
            let playback = frame
                .as_ref()
                .filter(|frame| frame.subject.ready)
                .map_or(&empty, |frame| frame.playback.as_ref());
            if frame.is_none() {
                window.set_tracks(Default::default());
                self.catalog.replace(Catalog::default());
            }
            presentation::render(
                window,
                playback,
                frame.as_ref().and_then(Frame::selected_track),
                &self.watches.borrow(),
                &properties::Values::new(),
            );
        }
        frame
    }
}

impl Catalog {
    fn update(
        &mut self,
        window: &MediaInspectorWindow,
        source: (usize, u64),
        tracks: &Arc<Vec<Track>>,
        ready: bool,
    ) -> Option<usize> {
        let tracks = if ready {
            tracks.clone()
        } else {
            Arc::default()
        };
        if self.source != Some(source)
            || (!Arc::ptr_eq(&self.tracks, &tracks) && self.tracks != tracks)
        {
            let selected = usize::try_from(window.get_selected_track()).ok();
            let preserve = (self.source == Some(source))
                .then(|| {
                    selected
                        .and_then(|index| self.order.get(index))
                        .and_then(|index| self.tracks.get(*index))
                        .map(track::Identity::from)
                })
                .flatten();
            let ordered = track::ordered(&tracks);
            let choice = preserve.and_then(|identity| {
                ordered
                    .iter()
                    .position(|(_, track)| track::Identity::from(*track) == identity)
            });
            let index = choice.or_else(|| (!ordered.is_empty()).then_some(0));
            self.order = ordered.iter().map(|(index, _)| *index).collect();
            let labels = ordered
                .iter()
                .map(|(_, track)| track::label(track).into())
                .collect();
            set_rows_if_changed(&window.get_tracks(), labels, |model| {
                window.set_tracks(model)
            });
            window.set_selected_track(
                index
                    .and_then(|index| i32::try_from(index).ok())
                    .unwrap_or(-1),
            );
            self.source = Some(source);
            self.tracks = tracks;
            self.revision = self.revision.wrapping_add(1);
        }
        usize::try_from(window.get_selected_track())
            .ok()
            .and_then(|index| self.order.get(index).copied())
    }
}
