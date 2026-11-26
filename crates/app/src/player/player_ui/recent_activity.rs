//! Keep Open Recent and the Welcome window in sync across independent players.
use super::media_actions::recent::{recent_open_menu_rows, refresh_welcome_history};
use crate::{
    PlayerWindow, WelcomeWindow, platform, playback_history::PlaybackHistoryState,
    playback_history_window::ApplicationHistory,
};
use riina_core::{Preferences, RecentMedia, sync::lock};
use riina_ui::{PlayerCatalogState, PlayerMenuActions, PlayerPreferences};
use slint::ComponentHandle;
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(super) struct ApplicationRecents {
    profiles: RefCell<HashMap<PathBuf, Rc<RecentActivity>>>,
}

impl ApplicationRecents {
    pub(super) fn for_profile(
        &self,
        path: &Path,
        prefs: &Preferences,
        use_system_services: bool,
    ) -> Rc<RecentActivity> {
        self.profiles
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| Rc::new(RecentActivity::new(path, prefs, use_system_services)))
            .clone()
    }
}

pub(super) struct RecentActivity {
    path: PathBuf,
    documents: Arc<Mutex<Vec<RecentMedia>>>,
    resume: RefCell<Vec<RecentMedia>>,
    revision: Cell<u64>,
    windows: RefCell<Vec<RecentWindow>>,
    history: OnceCell<RecentHistory>,
    use_system_services: bool,
}

struct RecentWindow {
    player: slint::Weak<PlayerWindow>,
    welcome: Option<slint::Weak<WelcomeWindow>>,
}

struct RecentHistory {
    owner: Weak<ApplicationHistory>,
    state: Arc<Mutex<PlaybackHistoryState>>,
}

impl RecentActivity {
    fn new(path: &Path, prefs: &Preferences, use_system_services: bool) -> Self {
        Self {
            path: path.to_path_buf(),
            documents: Arc::new(Mutex::new(prefs.recent_open_documents.clone())),
            resume: RefCell::new(prefs.recent_media.clone()),
            revision: Cell::new(prefs.recent_resume_revision),
            windows: RefCell::new(Vec::new()),
            history: OnceCell::new(),
            use_system_services,
        }
    }

    pub(super) fn documents(&self) -> &Arc<Mutex<Vec<RecentMedia>>> {
        &self.documents
    }

    pub(super) fn bind_history(
        &self,
        owner: &Rc<ApplicationHistory>,
        state: &Arc<Mutex<PlaybackHistoryState>>,
    ) {
        self.history.get_or_init(|| RecentHistory {
            owner: Rc::downgrade(owner),
            state: state.clone(),
        });
    }

    pub(super) fn attach(&self, app: &PlayerWindow, welcome: Option<&WelcomeWindow>) {
        self.windows.borrow_mut().push(RecentWindow {
            player: app.as_weak(),
            welcome: welcome.map(ComponentHandle::as_weak),
        });
        self.refresh();
    }

    pub(super) fn refresh_welcome(&self, app: &PlayerWindow, welcome: &WelcomeWindow) {
        let settings = app.global::<PlayerPreferences>();
        let documents = lock(&self.documents).clone();
        refresh_welcome_history(
            welcome,
            &self.resume.borrow(),
            &documents,
            settings.get_record_recent_files(),
            settings.get_resume_last_position(),
        );
    }

    pub(super) fn welcome_selection(
        &self,
        app: &PlayerWindow,
        token: &str,
        resume_selected: bool,
    ) -> Option<RecentMedia> {
        let prefs = app.global::<PlayerPreferences>();
        if !prefs.get_record_recent_files() {
            return None;
        }
        let resume = self.resume.borrow();
        let last =
            super::media_actions::recent::resumable(&resume, prefs.get_resume_last_position());
        if resume_selected {
            return last.filter(|entry| entry.path == token).cloned();
        }
        super::media_actions::recent::visible_documents(&lock(&self.documents), last)
            .find(|entry| entry.path == token)
            .cloned()
    }

    pub(super) fn document(&self, path: &str) -> Option<RecentMedia> {
        lock(&self.documents)
            .iter()
            .find(|item| item.path == path)
            .cloned()
    }

    pub(super) fn remember_document(&self, app: &PlayerWindow, path: String, title: String) {
        if app.global::<PlayerPreferences>().get_record_recent_files()
            && let Err(error) = self.record_documents([(path, title)])
        {
            crate::diagnostics::report(app, error);
        }
    }

    pub(super) fn record_selections(
        &self,
        paths: impl IntoIterator<Item = String>,
    ) -> Result<(), String> {
        self.record_documents(paths.into_iter().map(|path| {
            let local = crate::file_actions::local_media_path(&path);
            let title = local
                .as_deref()
                .and_then(Path::file_name)
                .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
            (path, title)
        }))
    }

    pub(super) fn record_documents(
        &self,
        documents: impl IntoIterator<Item = (String, String)>,
    ) -> Result<(), String> {
        let documents = documents
            .into_iter()
            .filter(|(path, _)| !path.is_empty())
            .map(|(path, title)| RecentMedia {
                path,
                title,
                position: 0.0,
            })
            .collect::<Vec<_>>();
        if documents.is_empty() {
            return Ok(());
        }
        let mut updated = None;
        crate::preference_store::update_automatically(
            &self.path,
            &Preferences::default(),
            |prefs| {
                if prefs.record_recent_files {
                    let recent = &mut prefs.recent_open_documents;
                    for document in &documents {
                        recent.retain(|item| item.path != document.path);
                        recent.insert(0, document.clone());
                        recent.truncate(10);
                    }
                    updated = Some(recent.clone());
                }
                Ok(())
            },
        )
        .map_err(|error| format!("Could not save Open Recent: {error}"))?;
        let Some(updated) = updated else {
            return Ok(());
        };
        *lock(&self.documents) = updated;
        self.refresh();
        let mut failure = self
            .sync_plugins()
            .err()
            .map(|error| format!("Could not update plugin recent files: {error}"));
        if self.use_system_services
            && let Err(error) = platform::remember_recent_documents(
                documents.iter().map(|entry| entry.path.as_str()),
            )
        {
            failure.get_or_insert_with(|| {
                format!("Could not update system recent documents: {error}")
            });
        }
        failure.map_or(Ok(()), Err)
    }

    pub(super) fn clear_documents(&self) -> Result<(), String> {
        crate::preference_store::update(&self.path, &Preferences::default(), |prefs| {
            prefs.recent_open_documents.clear();
            Ok(())
        })?;
        lock(&self.documents).clear();
        self.refresh();
        let system = self.clear_system_documents();
        system.and(self.sync_plugins())
    }

    pub(super) fn clear_for_history(&self) -> Result<(), String> {
        let mut revision = self.revision.get();
        crate::preference_store::update(&self.path, &Preferences::default(), |prefs| {
            prefs.recent_open_documents.clear();
            prefs.recent_media.clear();
            revision = prefs.recent_resume_revision.wrapping_add(1);
            prefs.recent_resume_revision = revision;
            Ok(())
        })
        .map_err(|error| format!("Could not clear recent files: {error}"))?;
        self.revision.set(revision);
        self.resume.borrow_mut().clear();
        lock(&self.documents).clear();
        self.refresh();
        self.clear_system_documents()
    }

    fn clear_system_documents(&self) -> Result<(), String> {
        if self.use_system_services {
            platform::clear_recent_documents().map_err(|error| {
                format!("Riina’s recent files were cleared, but the system list could not be cleared: {error}")
            })
        } else {
            Ok(())
        }
    }

    pub(super) fn revision(&self) -> u64 {
        if let Ok(Some(saved)) = crate::preference_load::read_valid(&self.path)
            && self.revision.replace(saved.recent_resume_revision) != saved.recent_resume_revision
        {
            self.resume.replace(saved.recent_media);
            *lock(&self.documents) = saved.recent_open_documents;
            self.refresh();
        }
        self.revision.get()
    }

    pub(super) fn save_resume(&self, media: RecentMedia, revision: u64) -> Result<(), String> {
        if self.revision.get() != revision {
            return Ok(());
        }
        let mut updated = None;
        crate::preference_store::update_automatically(
            &self.path,
            &Preferences::default(),
            |prefs| {
                if prefs.recent_resume_revision == revision && prefs.resume_last_position {
                    prefs.remember_media(media);
                    updated = Some(prefs.recent_media.clone());
                }
                Ok(())
            },
        )?;
        if let Some(updated) = updated {
            *self.resume.borrow_mut() = updated;
            self.refresh();
        }
        Ok(())
    }

    fn sync_plugins(&self) -> Result<(), String> {
        if let Some(history) = self.history.get()
            && let Some(owner) = history.owner.upgrade()
        {
            let entries = lock(&history.state).entries().to_vec();
            owner.sync_plugins(&entries)?;
        }
        Ok(())
    }

    fn refresh(&self) {
        let windows = {
            let mut registered = self.windows.borrow_mut();
            let mut windows = Vec::new();
            registered.retain(|window| {
                let Some(app) = window.player.upgrade() else {
                    return false;
                };
                windows.push((app, window.welcome.as_ref().and_then(slint::Weak::upgrade)));
                true
            });
            windows
        };
        let rows = recent_open_menu_rows(&lock(&self.documents));
        for (app, welcome) in windows {
            app.global::<PlayerCatalogState>()
                .set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(
                    rows.clone(),
                )));
            app.global::<PlayerMenuActions>()
                .invoke_video_menu_changed();
            if let Some(welcome) = welcome {
                self.refresh_welcome(&app, &welcome);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    #[test]
    fn windowless_document_updates_share_order_and_clear_separately_from_resume() {
        let directory = TempDir::new("windowless-recent-documents").unwrap();
        let path = directory.path().join("preferences.json");
        let resume = RecentMedia {
            path: "resume.mkv".into(),
            title: "Resume".into(),
            position: 34.0,
        };
        let prefs = Preferences {
            recent_media: vec![resume.clone()],
            ..Preferences::default()
        };
        std::fs::write(&path, serde_json::to_vec(&prefs).unwrap()).unwrap();
        let owner = ApplicationRecents::default();
        let recent = owner.for_profile(&path, &prefs, false);
        recent
            .record_documents([
                ("café.mkv".into(), "First title".into()),
                ("second.mkv".into(), "Second".into()),
                ("café.mkv".into(), "Updated title".into()),
            ])
            .unwrap();
        let player_recents = owner.for_profile(&path, &prefs, false);
        assert!(Rc::ptr_eq(&recent, &player_recents));
        let saved = crate::preference_load::read_valid(&path).unwrap().unwrap();
        assert_eq!(
            *lock(player_recents.documents()),
            saved.recent_open_documents
        );
        assert_eq!(saved.recent_open_documents.len(), 2);
        assert_eq!(saved.recent_open_documents[0].title, "Updated title");
        assert_eq!(saved.recent_open_documents[1].path, "second.mkv");

        recent.clear_documents().unwrap();
        let cleared = crate::preference_load::read_valid(&path).unwrap().unwrap();
        assert!(cleared.recent_open_documents.is_empty());
        assert!(lock(player_recents.documents()).is_empty());
        assert_eq!(cleared.recent_media, [resume]);
    }

    #[test]
    fn a_resume_write_requires_the_persisted_clear_revision() {
        let directory = TempDir::new("resume-revision").unwrap();
        let path = directory.path().join("preferences.json");
        let prefs = Preferences {
            resume_last_position: true,
            ..Preferences::default()
        };
        std::fs::write(&path, serde_json::to_vec(&prefs).unwrap()).unwrap();
        let older_player = RecentActivity::new(&path, &prefs, false);
        let separate_player = RecentActivity::new(&path, &prefs, false);
        separate_player.clear_for_history().unwrap();
        let media = RecentMedia {
            path: "playback.mkv".into(),
            title: "Playback".into(),
            position: 12.0,
        };
        older_player.save_resume(media.clone(), 0).unwrap();
        let after_clear = crate::preference_load::read_valid(&path).unwrap().unwrap();
        assert!(after_clear.recent_media.is_empty());

        let new_revision = older_player.revision();
        assert_eq!(new_revision, 1);
        older_player
            .save_resume(media.clone(), new_revision)
            .unwrap();
        let reopened = crate::preference_load::read_valid(&path).unwrap().unwrap();
        assert_eq!(reopened.recent_media, [media]);
    }
}
