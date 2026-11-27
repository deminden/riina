//! Project native cache and seek observations into the owning player's OSD.
use crate::{PlayerWindow, localized_number};
use riina_core::{Phase, Playback, PlaybackCache, media_reference::is_network_url_candidate};
use riina_ui::PlayerOsdState;
use slint::ComponentHandle;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Opening,
    Waiting {
        buffering: bool,
        seeking: bool,
        cache: PlaybackCache,
    },
}

fn activity(playback: &Playback) -> Option<Activity> {
    if !matches!(
        playback.phase,
        Phase::Loading | Phase::Buffering | Phase::Seeking
    ) || !playback.playlist.iter().any(|entry| {
        Some(entry.id) == playback.active_playlist_entry
            && is_network_url_candidate(&entry.filename)
    }) {
        return None;
    }
    if playback.phase == Phase::Loading {
        Some(Activity::Opening)
    } else {
        Some(Activity::Waiting {
            buffering: playback.buffering,
            seeking: playback.seeking,
            cache: playback.cache,
        })
    }
}

pub(super) fn update(app: &PlayerWindow, playback: &Playback, previous: Option<&Playback>) {
    let current = activity(playback);
    if previous.is_some_and(|previous| activity(previous) == current) {
        return;
    }
    let (buffering, seeking, cache) = match current {
        Some(Activity::Waiting {
            buffering,
            seeking,
            cache,
        }) => (buffering, seeking, Some(cache)),
        _ => (false, false, None),
    };
    let osd = app.global::<PlayerOsdState>();
    osd.set_stream_opening(current == Some(Activity::Opening));
    osd.set_stream_buffering(buffering);
    osd.set_stream_seeking(seeking);
    let percent = cache
        .filter(|_| buffering)
        .and_then(|cache| cache.fill_percent)
        .map(|percent| localized_number::format_delay(f64::from(percent)));
    osd.set_stream_fill_percent(percent.unwrap_or_default().into());
    osd.set_stream_cached_amount(
        cache
            .map_or_else(String::new, |cache| {
                localized_number::format_cache_amount(cache.forward_bytes, true)
            })
            .into(),
    );
    osd.set_stream_input_rate(
        cache
            .map_or_else(String::new, |cache| {
                localized_number::format_cache_amount(cache.input_bytes_per_second, false)
            })
            .into(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_ui::{PlayerMusicModeState, PlayerPreferences};
    use std::sync::Arc;

    #[test]
    fn stream_activity_uses_native_state_and_live_display_preferences()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let osd = app.global::<PlayerOsdState>();
        let preferences = app.global::<PlayerPreferences>();
        let mut state = Playback {
            phase: Phase::Loading,
            active_playlist_entry: Some(2),
            playlist: Arc::new(vec![riina_core::PlaylistEntry {
                id: 2,
                filename: "http://127.0.0.1/clip.mp4".into(),
                title: None,
                current: false,
                playing: false,
            }]),
            ..Playback::default()
        };
        update(&app, &state, None);
        assert!(osd.get_stream_activity_visible());
        assert_eq!(osd.get_stream_activity_label(), "Opening stream…");
        assert_eq!(osd.get_stream_activity_detail(), "");

        state.phase = Phase::Buffering;
        state.buffering = true;
        state.cache = PlaybackCache {
            fill_percent: Some(37),
            forward_bytes: 2048,
            input_bytes_per_second: 3000,
        };
        update(&app, &state, None);
        assert_eq!(osd.get_stream_activity_label(), "Buffering… 37%");
        assert_eq!(osd.get_stream_activity_detail(), "2 KiB (3 KB/s)");
        preferences.set_network_show_buffering_throbber(false);
        assert!(!osd.get_stream_activity_visible());

        state.phase = Phase::Seeking;
        state.buffering = false;
        state.seeking = true;
        update(&app, &state, None);
        assert!(osd.get_stream_activity_visible());
        assert_eq!(osd.get_stream_activity_label(), "Seeking…");
        preferences.set_network_show_seeking_throbber(false);
        assert!(!osd.get_stream_activity_visible());
        preferences.set_network_show_seeking_throbber(true);
        app.global::<PlayerMusicModeState>().set_music_mode(true);
        assert!(!osd.get_stream_activity_visible());
        app.global::<PlayerMusicModeState>().set_music_mode(false);

        let previous = state.clone();
        state.phase = Phase::Stopped;
        update(&app, &state, Some(&previous));
        assert!(!osd.get_stream_activity_visible());
        assert_eq!(osd.get_stream_activity_detail(), "");
        state.phase = Phase::Loading;
        state.active_playlist_entry = Some(3);
        update(&app, &state, None);
        assert!(!osd.get_stream_activity_visible());
        Ok(())
    }
}
