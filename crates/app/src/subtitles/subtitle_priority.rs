//! Apply subtitle filename priorities after libmpv has discovered sidecar tracks.

use riina_core::{
    Phase, Playback, TrackKind,
    subtitle_controls::{AutoLoadMode, AutoLoadPreferences},
};
use std::path::Path;

pub(crate) fn priority_occurrences(path: &Path, keywords: &[String]) -> usize {
    let filename = path
        .file_stem()
        .or_else(|| path.file_name())
        .unwrap_or_default()
        .to_string_lossy();
    keywords
        .iter()
        .map(|keyword| filename.match_indices(keyword).count())
        .sum()
}

#[derive(Default)]
pub(crate) struct AutoPriority {
    generation: Option<u64>,
    preferences: String,
    mode: AutoLoadMode,
    inspected_tracks: Vec<(i64, String)>,
}

impl AutoPriority {
    pub(crate) fn observe(
        &mut self,
        state: &Playback,
        preferences: &AutoLoadPreferences,
    ) -> Option<i64> {
        if self.generation != Some(state.generation)
            || self.preferences != preferences.priority_keywords
            || self.mode != preferences.mode
        {
            self.generation = Some(state.generation);
            self.preferences.clone_from(&preferences.priority_keywords);
            self.mode = preferences.mode;
            self.inspected_tracks.clear();
        }
        if preferences.mode == AutoLoadMode::Disabled
            || matches!(state.phase, Phase::Stopped | Phase::Loading | Phase::Failed)
        {
            return None;
        }

        let tracks = state
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Sub && track.external)
            .filter_map(|track| {
                track
                    .external_filename
                    .as_ref()
                    .map(|filename| (track.id, filename.clone(), track.is_primary_selected()))
            })
            .collect::<Vec<_>>();
        let signature = tracks
            .iter()
            .map(|(id, filename, _)| (*id, filename.clone()))
            .collect::<Vec<_>>();
        if signature.is_empty() || signature == self.inspected_tracks {
            return None;
        }
        self.inspected_tracks = signature;

        let keywords = preferences
            .priority_keywords
            .split(',')
            .map(str::trim)
            .filter(|keyword| !keyword.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut best = None::<(usize, i64, bool)>;
        for (id, filename, selected) in tracks {
            let score = priority_occurrences(Path::new(&filename), &keywords);
            if score == 0 {
                continue;
            }
            if best.is_none_or(|(best_score, _, _)| score > best_score) {
                best = Some((score, id, selected));
            }
        }
        best.and_then(|(_, id, selected)| (!selected).then_some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::{AutoPriority, priority_occurrences};
    use riina_core::{
        Phase, Playback, Track, TrackKind,
        subtitle_controls::{AutoLoadMode, AutoLoadPreferences},
    };
    use std::path::Path;

    fn track(id: i64, filename: &str, selected: bool) -> Track {
        Track {
            id,
            kind: TrackKind::Sub,
            title: None,
            lang: None,
            codec: None,
            selected,
            main_selection: Some(0),
            external: true,
            albumart: false,
            external_filename: Some(filename.into()),
            is_default: false,
            forced: false,
            channels: None,
            channel_layout: None,
            sample_rate: None,
            width: None,
            height: None,
            frame_rate: None,
        }
    }

    fn preferences(priority_keywords: &str) -> AutoLoadPreferences {
        AutoLoadPreferences {
            mode: AutoLoadMode::Intelligent,
            priority_keywords: priority_keywords.into(),
            search_paths: "./*".into(),
        }
    }

    #[test]
    fn leaves_default_track_selection_alone_without_keyword_matches() {
        let state = Playback {
            generation: 2,
            phase: Phase::Playing,
            tracks: vec![
                track(7, "Movie.en.srt", false),
                track(8, "Movie.fr.srt", false),
            ],
            ..Playback::default()
        };
        assert_eq!(
            AutoPriority::default().observe(&state, &preferences("")),
            None
        );
    }

    #[test]
    fn priority_keyword_selects_best_match_and_preserves_existing_selection() {
        let state = Playback {
            generation: 2,
            phase: Phase::Playing,
            tracks: vec![
                track(7, "Movie.en.srt", false),
                track(8, "Movie.en.forced.srt", false),
            ],
            ..Playback::default()
        };
        assert_eq!(
            AutoPriority::default().observe(&state, &preferences("forced")),
            Some(8)
        );

        let selected = Playback {
            tracks: vec![track(7, "Movie.en.srt", true)],
            ..state
        };
        assert_eq!(
            AutoPriority::default().observe(&selected, &preferences("")),
            None
        );
    }

    #[test]
    fn priority_keywords_follow_case_sensitive_filename_stem_matching() {
        let state = Playback {
            generation: 2,
            phase: Phase::Playing,
            tracks: vec![
                track(7, "C:/Subtitles/forced/Movie.en.srt", false),
                track(8, "C:/Subtitles/Movie.en.FORCED.srt", false),
                track(9, "C:/Subtitles/Movie.en.forced.srt", false),
            ],
            ..Playback::default()
        };
        assert_eq!(
            AutoPriority::default().observe(&state, &preferences("forced")),
            Some(9)
        );
    }

    #[test]
    fn priority_occurrences_ignore_directories_and_extensions() {
        let keywords = vec!["forced".to_owned(), "srt".to_owned()];
        assert_eq!(
            priority_occurrences(Path::new("C:/forced/Movie.en.srt"), &keywords),
            0
        );
        assert_eq!(
            priority_occurrences(Path::new("C:/captions/Movie.en.FORCED.srt"), &keywords),
            0
        );
        assert_eq!(
            priority_occurrences(Path::new("C:/captions/Movie.en.forced.srt"), &keywords),
            1
        );
    }
}
