use std::path::Path;

pub(super) const MIME: &str = "text/uri-list";

pub(super) fn encode(paths: &[String]) -> Option<String> {
    let mut list = String::new();
    for path in paths {
        let url = url::Url::from_file_path(Path::new(path)).ok()?;
        list.push_str(url.as_str());
        list.push_str("\r\n");
    }
    Some(list)
}

pub(super) fn decode(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| {
            line.split_once(':')
                .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
                || riina_core::media_reference::validate_network_url(line).is_ok()
        })
        .flat_map(riina_core::playlist::clipboard_references)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn uri_list_round_trip_preserves_special_filename_bytes_and_order() {
        let paths = vec!["/media/a %é#\r".into(), "/media/b\nclip.mp4".into()];
        let encoded = encode(&paths).unwrap();
        assert_eq!(
            encoded,
            "file:///media/a%20%25%C3%A9%23%0D\r\nfile:///media/b%0Aclip.mp4\r\n"
        );
        assert_eq!(decode(&encoded), paths);
    }

    #[test]
    fn uri_list_ignores_comments_relative_paths_and_unsupported_file_authority() {
        assert_eq!(
            decode(
                "# File manager\r\nfile://host/folder\r\nrelative.txt\n\
                 FILE://localhost/media/a%23b.mkv\r\nhttps://example.com/video.mp4\n",
            ),
            ["/media/a#b.mkv", "https://example.com/video.mp4"]
        );
    }
}
