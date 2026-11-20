//! Match episode subtitles through recursively detected filename-series prefixes.

use crate::subtitle_discovery::edit_distance_bounded;
use std::collections::BTreeMap;
use std::path::PathBuf;
use unicode_segmentation::UnicodeSegmentation;

const MAX_SERIES_GROUP_PAIRS: usize = 10_000;
const MAX_SERIES_PREFIX_GRAPHEMES: usize = 256;
const CHINESE_NUMBER_CHARACTERS: &str = "零一二三四五六七八九十";

struct SeriesGroup {
    prefix: String,
    members: Vec<usize>,
}

pub(crate) fn matched_subtitles_for(
    media_stem: &str,
    sibling_media_stems: &[String],
    subtitles: &[(PathBuf, String)],
) -> Vec<PathBuf> {
    let mut media_stems = sibling_media_stems.to_vec();
    if !media_stems.iter().any(|stem| stem == media_stem) {
        media_stems.push(media_stem.to_owned());
    }
    media_stems.sort();
    media_stems.dedup();

    let Some(current_media_index) = media_stems.iter().position(|stem| stem == media_stem) else {
        return Vec::new();
    };
    let media_groups = group_series_names(&media_stems);
    let Some(current_group_index) = media_groups
        .iter()
        .position(|group| group.members.len() > 2 && group.members.contains(&current_media_index))
    else {
        return Vec::new();
    };
    let subtitle_stems = subtitles
        .iter()
        .map(|(_, stem)| stem.clone())
        .collect::<Vec<_>>();
    let subtitle_groups = group_series_names(&subtitle_stems);
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
    let current_media_series_index = media_series
        .iter()
        .position(|(group_index, _)| *group_index == current_group_index);
    let Some(current_media_series_index) = current_media_series_index else {
        return Vec::new();
    };
    let current_media_group = media_series[current_media_series_index].1;
    let current_media_prefix = current_media_group.prefix.as_str();
    let closest_subtitle = nearest_group(subtitle_series.iter().enumerate().filter_map(
        |(subtitle_index, (_, group))| {
            distances[current_media_series_index][subtitle_index]
                .map(|distance| (distance, group.prefix.as_str(), subtitle_index))
        },
    ));
    let Some((distance, _, subtitle_series_index)) = closest_subtitle else {
        return Vec::new();
    };
    let subtitle_group_index = subtitle_series[subtitle_series_index].0;
    let subtitle_group = &subtitle_groups[subtitle_group_index];
    let threshold = (current_media_prefix.graphemes(true).count()
        + subtitle_group.prefix.graphemes(true).count())
        * 3
        / 5;
    if distance >= threshold
        || nearest_media_for_subtitle[subtitle_series_index]
            .is_none_or(|(_, _, nearest_index)| nearest_index != current_media_series_index)
    {
        return Vec::new();
    }

    let media_name = series_name_in_group(
        &media_stems[current_media_index],
        &current_media_group.prefix,
    );
    subtitle_group
        .members
        .iter()
        .filter_map(|subtitle_index| {
            let (_, subtitle_stem) = &subtitles[*subtitle_index];
            let subtitle_name = series_name_in_group(subtitle_stem, &subtitle_group.prefix);
            same_series_name(&media_name, &subtitle_name)
                .then(|| subtitles[*subtitle_index].0.clone())
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

fn group_series_names(stems: &[String]) -> Vec<SeriesGroup> {
    let graphemes = stems
        .iter()
        .map(|stem| stem.graphemes(true).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let members = (0..stems.len()).collect::<Vec<_>>();
    let mut groups = Vec::new();
    split_series_group(&members, &graphemes, Vec::new(), &mut groups);
    groups
}

fn split_series_group(
    members: &[usize],
    graphemes: &[Vec<&str>],
    mut prefix: Vec<String>,
    result: &mut Vec<SeriesGroup>,
) {
    if members.len() < 3 {
        result.push(SeriesGroup {
            prefix: prefix.into_iter().collect(),
            members: members.to_vec(),
        });
        return;
    }

    let mut offset = prefix.len();
    loop {
        let mut branches = BTreeMap::<Option<String>, Vec<usize>>::new();
        for member in members {
            let branch = graphemes[*member]
                .get(offset)
                .map(|value| (*value).to_owned());
            branches.entry(branch).or_default().push(*member);
        }
        if branches.len() == 1 {
            if let Some(Some(next)) = branches.keys().next() {
                prefix.push(next.clone());
                offset += 1;
                continue;
            }
            break;
        }

        let largest_branch = branches.values().map(Vec::len).max().unwrap_or_default();
        let mut chinese_number_count = 0;
        let stop_for_number = branches.keys().any(|branch| {
            branch.as_deref().is_some_and(|value| {
                if value.chars().any(|character| character.is_ascii_digit()) {
                    return true;
                }
                if value
                    .chars()
                    .any(|character| CHINESE_NUMBER_CHARACTERS.contains(character))
                {
                    chinese_number_count += 1;
                }
                chinese_number_count >= 3
            })
        });
        if stop_for_number || largest_branch < 3 {
            break;
        }

        for (branch, children) in branches {
            let mut child_prefix = prefix.clone();
            if let Some(branch) = branch {
                child_prefix.push(branch);
            }
            split_series_group(&children, graphemes, child_prefix, result);
        }
        return;
    }

    result.push(SeriesGroup {
        prefix: prefix.into_iter().collect(),
        members: members.to_vec(),
    });
}

fn series_name_in_group(stem: &str, prefix: &str) -> String {
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
    name
}

fn same_series_name(left: &str, right: &str) -> bool {
    left == right
        || left
            .parse::<i64>()
            .ok()
            .zip(right.parse::<i64>().ok())
            .is_some_and(|(left, right)| left == right)
}

#[cfg(test)]
mod tests {
    use super::{group_series_names, matched_subtitles_for, series_name_in_group};
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
        assert_eq!(series_name_in_group(&stems[1], &groups[0].prefix), "2");
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

        assert_eq!(
            matched_subtitles_for(&media[1], &media, &subtitles),
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
        assert!(matched_subtitles_for(&media[0], &media, &subtitles).is_empty());
    }
}
