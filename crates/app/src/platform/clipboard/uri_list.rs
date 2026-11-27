use riina_core::stream_address::explicit_stream_url;
use std::path::Path;

pub(super) const MIME: &str = "text/uri-list";

pub(super) fn encode(references: &[String]) -> Option<String> {
    let mut list = String::new();
    for reference in references {
        if let Some(url) = explicit_stream_url(reference) {
            list.push_str(&url);
        } else {
            let path = url::Url::from_file_path(Path::new(reference)).ok()?;
            list.push_str(path.as_str());
        }
        list.push_str("\r\n");
    }
    Some(list)
}

pub(super) fn decode(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| crate::platform::desktop_drop::parse_reference(line, false))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn uri_list_round_trip_preserves_special_filename_bytes_and_order() {
        let references = vec![
            "/media/a %é#\r".into(),
            "https://example.test/片 one.mkv?sig=A%2fb+Z".into(),
            "/media/b\nclip.mp4".into(),
        ];
        let encoded = encode(&references).unwrap();
        assert_eq!(
            encoded,
            concat!(
                "file:///media/a%20%25%C3%A9%23%0D\r\n",
                "https://example.test/%E7%89%87%20one.mkv?sig=A%2fb+Z\r\n",
                "file:///media/b%0Aclip.mp4\r\n"
            )
        );
        assert_eq!(
            decode(&encoded),
            [
                "/media/a %é#\r",
                "https://example.test/%E7%89%87%20one.mkv?sig=A%2fb+Z",
                "/media/b\nclip.mp4"
            ]
        );
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
