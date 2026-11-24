//! File series shared by folder playlists and automatic subtitle matching.

use std::collections::BTreeMap;
use unicode_segmentation::UnicodeSegmentation;

const CHINESE_NUMBER_CHARACTERS: &str = "零一二三四五六七八九十";

pub(crate) struct SeriesGroup {
    pub(crate) prefix: String,
    pub(crate) members: Vec<usize>,
}

pub(crate) fn group_series_names(stems: &[String]) -> Vec<SeriesGroup> {
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
