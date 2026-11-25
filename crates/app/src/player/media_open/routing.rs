//! Keep each selected reference intact until its destination window is chosen.
use super::same_media;
use crate::file_selection;

pub(in crate::player) struct PlayerSelection {
    pub(in crate::player) paths: Vec<String>,
    pub(in crate::player) duplicate_reference: Option<String>,
    pub(in crate::player) explicit_playlist: bool,
}

pub(super) fn prepare_new_players(
    references: Vec<String>,
    preferences: &riina_core::Preferences,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<PlayerSelection>, String> {
    if preferences.group_simultaneous_opens_in_playlist && references.len() > 1 {
        let paths = file_selection::prepare_cancellable(references, true, &cancelled)?;
        return Ok((!paths.is_empty())
            .then_some(PlayerSelection {
                paths,
                duplicate_reference: None,
                explicit_playlist: false,
            })
            .into_iter()
            .collect());
    }

    let mut players: Vec<PlayerSelection> = Vec::new();
    for reference in references {
        if cancelled() {
            return Err("Media selection was cancelled".into());
        }
        if !preferences.allow_duplicate_players
            && players.iter().any(|player| {
                player
                    .duplicate_reference
                    .as_deref()
                    .is_some_and(|open| same_media(open, &reference))
            })
        {
            continue;
        }
        let paths = file_selection::prepare_cancellable(vec![reference.clone()], true, &cancelled)?;
        if !paths.is_empty() {
            players.push(PlayerSelection {
                paths,
                duplicate_reference: Some(reference),
                explicit_playlist: false,
            });
        }
    }
    Ok(players)
}

pub(crate) fn duplicate_player(
    reference: &str,
    allow_duplicate_players: bool,
    current_media: impl Iterator<Item = Option<String>>,
) -> Option<usize> {
    if reference.is_empty() || allow_duplicate_players {
        return None;
    }
    current_media.enumerate().find_map(|(index, media)| {
        media
            .filter(|open| same_media(open, reference))
            .map(|_| index)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::Preferences;

    #[test]
    fn separate_opens_keep_each_folder_and_playlist_in_one_window() {
        let scratch = riina_test_support::TempDir::new("open-reference-groups").unwrap();
        let path = |name: &str| scratch.path().join(name).to_string_lossy().into_owned();
        let folder = path("folder");
        let playlist = path("list.m3u");
        let clip = |name: &str| {
            std::path::Path::new(&folder)
                .join(name)
                .to_string_lossy()
                .into_owned()
        };
        let clip2 = clip("clip2.avi");
        let clip10 = clip("clip10.mkv");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(&clip10, []).unwrap();
        std::fs::write(&clip2, []).unwrap();
        std::fs::write(&playlist, "clip.avi\n").unwrap();

        let requests = vec![playlist.clone(), folder.clone(), folder.clone()];
        let mut prefs = Preferences::default();
        let separate = prepare_new_players(requests.clone(), &prefs, || false).unwrap();
        assert_eq!(separate.len(), 2);
        assert_eq!(separate[0].paths, std::slice::from_ref(&playlist));
        assert_eq!(separate[1].paths, [clip2.clone(), clip10.clone()]);
        assert_eq!(
            separate[1].duplicate_reference.as_deref(),
            Some(folder.as_str())
        );

        prefs.allow_duplicate_players = true;
        assert_eq!(
            prepare_new_players(requests.clone(), &prefs, || false)
                .unwrap()
                .len(),
            3
        );

        prefs.group_simultaneous_opens_in_playlist = true;
        let grouped = prepare_new_players(requests, &prefs, || false).unwrap();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].paths, [clip2, clip10]);
        assert_eq!(grouped[0].duplicate_reference, None);

        let single = prepare_new_players(vec![playlist], &prefs, || false).unwrap();
        assert!(single[0].duplicate_reference.is_some());
    }

    #[test]
    fn a_grouped_request_with_one_playable_file_still_bypasses_duplicate_filtering() {
        let prefs = Preferences {
            group_simultaneous_opens_in_playlist: true,
            ..Preferences::default()
        };
        let groups =
            prepare_new_players(vec!["list.m3u".into(), "clip.avi".into()], &prefs, || false)
                .unwrap();
        assert_eq!(groups[0].paths, ["clip.avi"]);
        assert_eq!(groups[0].duplicate_reference, None);

        let open = || [None, Some("clip.avi".into())].into_iter();
        assert_eq!(duplicate_player("clip.avi", false, open()), Some(1));
        assert_eq!(duplicate_player("clip.avi", true, open()), None);
        assert_eq!(duplicate_player("", false, open()), None);
    }
}
