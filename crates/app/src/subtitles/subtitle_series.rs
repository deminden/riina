//! Match episode subtitles through recursively detected filename-series prefixes.

use crate::series_names::{SeriesGroup, group_series_names};
use crate::subtitle_discovery::edit_distance_bounded;
use std::{collections::HashMap, path::PathBuf};
use unicode_segmentation::UnicodeSegmentation;

const MAX_SERIES_GROUP_PAIRS: usize = 10_000;
const MAX_SERIES_PREFIX_GRAPHEMES: usize = 256;

pub(crate) struct SeriesMatches<'a> {
    subtitles: &'a [(PathBuf, String)],
    media: HashMap<&'a str, (usize, EpisodeName)>,
    episodes: Vec<HashMap<EpisodeName, Vec<usize>>>,
}

impl<'a> SeriesMatches<'a> {
    pub(crate) fn new(media_stems: &'a [String], subtitles: &'a [(PathBuf, String)]) -> Self {
        let media_groups = group_series_names(media_stems);
        let subtitle_stems: Vec<_> = subtitles.iter().map(|(_, stem)| stem.clone()).collect();
        let subtitle_groups = group_series_names(&subtitle_stems);
        let mut media = HashMap::new();
        let mut episodes = Vec::new();
        for (media_index, subtitle_index) in pair_groups(&media_groups, &subtitle_groups) {
            let media_group = &media_groups[media_index];
            let subtitle_group = &subtitle_groups[subtitle_index];
            let mut by_episode: HashMap<_, Vec<_>> = HashMap::new();
            for member in &subtitle_group.members {
                let episode = series_name_in_group(&subtitles[*member].1, &subtitle_group.prefix);
                by_episode.entry(episode).or_default().push(*member);
            }
            for member in &media_group.members {
                let stem = media_stems[*member].as_str();
                let episode = series_name_in_group(stem, &media_group.prefix);
                media.insert(stem, (episodes.len(), episode));
            }
            episodes.push(by_episode);
        }
        Self {
            subtitles,
            media,
            episodes,
        }
    }

    pub(crate) fn for_media(&self, media_stem: &str) -> impl Iterator<Item = PathBuf> {
        self.media
            .get(media_stem)
            .and_then(|(pair, episode)| self.episodes[*pair].get(episode))
            .into_iter()
            .flatten()
            .map(|index| self.subtitles[*index].0.clone())
    }
}

fn pair_groups(
    media_groups: &[SeriesGroup],
    subtitle_groups: &[SeriesGroup],
) -> Vec<(usize, usize)> {
    let media_series = media_groups
        .iter()
        .enumerate()
        .filter(|(_, group)| {
            group.members.len() > 2
                && group.prefix.graphemes(true).count() <= MAX_SERIES_PREFIX_GRAPHEMES
        })
        .collect::<Vec<_>>();
    let subtitle_series = subtitle_groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.prefix.graphemes(true).count() <= MAX_SERIES_PREFIX_GRAPHEMES)
        .collect::<Vec<_>>();
    if media_series.is_empty()
        || subtitle_series.is_empty()
        || media_series.len().saturating_mul(subtitle_series.len()) > MAX_SERIES_GROUP_PAIRS
    {
        return Vec::new();
    }

    let largest_threshold = media_series
        .iter()
        .flat_map(|(_, media)| {
            subtitle_series.iter().map(|(_, subtitle)| {
                (media.prefix.graphemes(true).count() + subtitle.prefix.graphemes(true).count()) * 3
                    / 5
            })
        })
        .max()
        .unwrap_or_default();
    let distance_limit = largest_threshold.saturating_sub(1);
    let mut distances = vec![vec![None; subtitle_series.len()]; media_series.len()];
    for (media_index, (_, media)) in media_series.iter().enumerate() {
        for (subtitle_index, (_, subtitle)) in subtitle_series.iter().enumerate() {
            distances[media_index][subtitle_index] =
                edit_distance_bounded(&media.prefix, &subtitle.prefix, distance_limit);
        }
    }

    let nearest_media_for_subtitle = (0..subtitle_series.len())
        .map(|subtitle_index| {
            nearest_group(media_series.iter().enumerate().filter_map(
                |(media_index, (_, group))| {
                    distances[media_index][subtitle_index]
                        .map(|distance| (distance, group.prefix.as_str(), media_index))
                },
            ))
        })
        .collect::<Vec<_>>();
    media_series
        .iter()
        .enumerate()
        .filter_map(|(media_index, (media_group, media))| {
            let (distance, prefix, subtitle_index) =
                nearest_group(subtitle_series.iter().enumerate().filter_map(
                    |(index, (_, group))| {
                        distances[media_index][index]
                            .map(|distance| (distance, group.prefix.as_str(), index))
                    },
                ))?;
            let threshold =
                (media.prefix.graphemes(true).count() + prefix.graphemes(true).count()) * 3 / 5;
            let (_, _, nearest_media) = nearest_media_for_subtitle[subtitle_index]?;
            (distance < threshold && nearest_media == media_index)
                .then_some((*media_group, subtitle_series[subtitle_index].0))
        })
        .collect()
}

fn nearest_group<'a>(
    groups: impl Iterator<Item = (usize, &'a str, usize)>,
) -> Option<(usize, &'a str, usize)> {
    groups.min_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(right.1))
            .then_with(|| left.2.cmp(&right.2))
    })
}

#[derive(Debug, Hash, PartialEq, Eq)]
enum EpisodeName {
    Number(i64),
    Text(String),
}

fn series_name_in_group(stem: &str, prefix: &str) -> EpisodeName {
    let stem = stem.graphemes(true).collect::<Vec<_>>();
    let prefix_len = prefix.graphemes(true).count();
    let suffix = if prefix_len < stem.len() {
        &stem[prefix_len..]
    } else {
        &stem[..]
    };
    let mut name = String::new();
    let mut has_digit = false;
    for grapheme in suffix {
        let is_digit = grapheme.chars().all(char::is_numeric);
        if is_digit {
            has_digit = true;
        } else if has_digit {
            break;
        }
        name.push_str(grapheme);
    }
    name.parse::<i64>()
        .map_or_else(|_| EpisodeName::Text(name), EpisodeName::Number)
}

#[cfg(test)]
mod tests {
    use super::{EpisodeName, SeriesMatches, group_series_names, series_name_in_group};
    use std::path::PathBuf;

    #[test]
    fn groups_recurse_through_text_branches_but_stop_before_numeric_episode_ids() {
        let stems = [
            "Show.Video.A.01",
            "Show.Video.A.02",
            "Show.Video.A.03",
            "Show.Video.B.01",
            "Show.Video.B.02",
            "Show.Video.B.03",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let groups = group_series_names(&stems);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].prefix, "Show.Video.A.0");
        assert_eq!(groups[0].members, [0, 1, 2]);
        assert_eq!(
            series_name_in_group(&stems[1], &groups[0].prefix),
            EpisodeName::Number(2)
        );
        assert_eq!(groups[1].prefix, "Show.Video.B.0");
        assert_eq!(groups[1].members, [3, 4, 5]);
    }

    #[test]
    fn series_prefixes_can_differ_when_mutual_nearest_group_and_episode_name_match() {
        let media = [
            "the other side - part 001",
            "the other side - part 002",
            "the other side - part 003",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let subtitles = [
            ("episode-1.srt", "the other side - english 001.en"),
            ("episode-2.srt", "the other side - english 002.en"),
            ("episode-3.srt", "the other side - english 003.en"),
        ]
        .into_iter()
        .map(|(path, stem)| (PathBuf::from(path), stem.to_lowercase()))
        .collect::<Vec<_>>();

        let matches = SeriesMatches::new(&media, &subtitles);
        assert_eq!(
            matches.for_media(&media[1]).collect::<Vec<_>>(),
            [PathBuf::from("episode-2.srt")]
        );
    }

    #[test]
    fn two_media_items_do_not_form_an_iina_series_group() {
        let media = ["Show Part 001", "Show Part 002"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let subtitles = vec![(
            PathBuf::from("Show English 001.srt"),
            "show english 001".to_owned(),
        )];
        assert!(
            SeriesMatches::new(&media, &subtitles)
                .for_media(&media[0])
                .next()
                .is_none()
        );
    }
}
