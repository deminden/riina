//! Bind shared discovery to the current platform's native filename comparator.
pub fn prepare(paths: Vec<String>, open_media: bool) -> Result<Vec<String>, String> {
    riina_core::file_selection::prepare(paths, open_media, crate::platform::natural_compare)
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_filename_order_is_used_for_shared_selection() {
        assert_eq!(
            super::prepare(
                vec!["media/clip10.mkv".into(), "media/clip2.mkv".into()],
                true,
            )
            .unwrap(),
            ["media/clip2.mkv", "media/clip10.mkv"]
        );
    }
}
