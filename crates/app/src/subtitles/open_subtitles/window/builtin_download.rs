//! Download selected built-in results and commit one media-scoped attachment.
use super::{Player, Request, SearchState};
use crate::OnlineSubtitlesWindow;
use crate::open_subtitles::{ResultRow, download, save_download};
use riina_core::online_subtitles::Provider;
use riina_mpv::CommandSender;
use slint::ComponentHandle;

pub(super) fn install(
    window: &OnlineSubtitlesWindow,
    state: &SearchState,
    owner: Player,
    player: &CommandSender,
) {
    let weak_window = window.as_weak();
    let state = state.clone();
    let sender = player.clone();
    window.on_download_selected(move || {
        let (Some(window), Some(request)) = (weak_window.upgrade(), state.request()) else {
            return;
        };
        if window.get_searching() || window.get_downloading() {
            return;
        }
        let Some(app) = request.check(&owner, &window) else {
            return;
        };
        let selected = state.selection.borrow().indices();
        let selected_rows = {
            let rows = riina_core::sync::lock(&state.rows);
            selected
                .iter()
                .filter_map(|index| rows.get(*index).cloned())
                .collect::<Vec<_>>()
        };
        if selected_rows.is_empty() {
            return;
        }
        let preferences = app.global::<riina_ui::PlayerPreferences>();
        let key = preferences.get_open_subtitles_api_key().to_string();
        let assrt_token = preferences.get_assrt_token().to_string();
        let total = selected_rows.len();
        window.set_downloading(true);
        let suffix = if total == 1 { "" } else { "s" };
        window.set_status_text(format!("Downloading {total} selected subtitle{suffix}…").into());
        let weak_window = window.as_weak();
        let owner = owner.clone();
        let sender = sender.clone();
        let started = crate::background::spawn_named("riina-subtitle-download", move || {
            let mut paths = Vec::with_capacity(total);
            let mut failure = None;
            for (index, row) in selected_rows.iter().enumerate() {
                if !request.active() {
                    return;
                }
                if !request
                    .context
                    .media
                    .is_observed(&riina_core::sync::lock(&owner.playback))
                {
                    break;
                }
                match download_row(row, &key, &assrt_token) {
                    Ok(path) => paths.push(path),
                    Err(error) => {
                        failure = Some(format!("Subtitle {} of {total}: {error}", index + 1));
                        break;
                    }
                }
            }
            let batch = Downloads {
                paths,
                total,
                failure,
            };
            let _ = crate::background::to_ui(weak_window, move |window| {
                batch.finish(&window, &request, &owner, &sender);
            });
        });
        if let Err(error) = started {
            window.set_downloading(false);
            window.set_status_text(format!("Could not start subtitle download: {error}").into());
        }
    });
}

struct Downloads {
    paths: Vec<String>,
    total: usize,
    failure: Option<String>,
}

impl Downloads {
    fn finish(
        self,
        window: &OnlineSubtitlesWindow,
        request: &Request,
        owner: &Player,
        sender: &CommandSender,
    ) {
        if !request.active() {
            return;
        }
        window.set_downloading(false);
        let Some(app) = request.check(owner, window) else {
            return;
        };
        let count = self.paths.len();
        if count != 0
            && let Err(error) = request.context.media.attach(sender, self.paths)
        {
            window.set_status_text(format!("Could not load downloaded subtitles: {error}").into());
            return;
        }
        if let Some(error) = self.failure {
            let total = self.total;
            window.set_status_text(
                format!("Downloaded and queued {count} of {total}. {error}").into(),
            );
            return;
        }
        let suffix = if count == 1 { "" } else { "s" };
        app.set_status(format!("Downloaded and queued {count} subtitle{suffix}.").into());
        let _ = window.hide();
    }
}

fn download_row(row: &ResultRow, key: &str, token: &str) -> Result<String, String> {
    let (name, bytes) = match row.provider {
        Provider::OpenSubtitles => download(key, row.file_id),
        Provider::Assrt => crate::assrt_subtitles::download(token, row.file_id),
        Provider::Shooter => {
            let url = row
                .download_url
                .as_deref()
                .ok_or("Shooter.cn result has no download link")?;
            crate::shooter_subtitles::download(
                row.file_id,
                row.release.trim_start_matches('.'),
                url,
            )
        }
    }?;
    save_download(&name, &bytes)?
        .into_os_string()
        .into_string()
        .map_err(|_| "The subtitle cache path contains invalid Unicode".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_downloads_never_attach_to_replacement_media_or_update_a_reopened_search()
    -> Result<(), slint::PlatformError> {
        use super::super::{SearchContext, media::Media};
        use crate::PlayerWindow;
        use riina_core::{Command, Phase, Playback, sync::lock};
        use std::sync::{Arc, Mutex, mpsc::TryRecvError};

        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let window = OnlineSubtitlesWindow::new()?;
        let playback = Playback {
            generation: 7,
            phase: Phase::Paused,
            ..Default::default()
        };
        let search = SearchState::default();
        let owner = Player {
            app: app.as_weak(),
            playback: Arc::new(Mutex::new(Arc::new(playback.clone()))),
        };
        *search.context.borrow_mut() = Some(SearchContext {
            media: Media::capture(&app, &playback).unwrap(),
            media_path: "episode.mkv".into(),
            languages: String::new(),
            provider: Provider::OpenSubtitles,
        });
        let request = search.request().unwrap();
        let (sender, engine) = CommandSender::recording(4);
        let batch = || Downloads {
            paths: vec!["download.srt".into()],
            total: 1,
            failure: None,
        };

        app.set_media_request_generation(1);
        batch().finish(&window, &request, &owner, &sender);
        assert_eq!(engine.commands.try_recv(), Err(TryRecvError::Empty));
        app.set_media_request_generation(0);
        Arc::make_mut(&mut lock(&owner.playback)).generation = 8;
        batch().finish(&window, &request, &owner, &sender);
        assert_eq!(engine.commands.try_recv(), Err(TryRecvError::Empty));
        Arc::make_mut(&mut lock(&owner.playback)).generation = 7;
        batch().finish(&window, &request, &owner, &sender);
        assert_eq!(
            engine.commands.try_recv(),
            Ok(Command::AddSubtitlesForMedia {
                generation: 7,
                paths: vec!["download.srt".into()],
            })
        );

        search.retire(&window);
        window.set_downloading(true);
        batch().finish(&window, &request, &owner, &sender);
        assert!(window.get_downloading());
        assert_eq!(engine.commands.try_recv(), Err(TryRecvError::Empty));
        Ok(())
    }
}
