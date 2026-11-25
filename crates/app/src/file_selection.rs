//! Bind shared discovery to the current platform's native filename comparator.
pub fn prepare(paths: Vec<String>, open_media: bool) -> Result<Vec<String>, String> {
    prepare_cancellable(paths, open_media, || false)
}

pub(crate) fn prepare_cancellable(
    paths: Vec<String>,
    open_media: bool,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<String>, String> {
    // Resolve macOS aliases before opening a folder or classifying its target.
    let paths = paths
        .into_iter()
        .map(|path| {
            if cancelled() {
                return Err("Media selection was cancelled".to_owned());
            }
            Ok(
                if path.is_empty()
                    || path.contains('\0')
                    || riina_core::media_reference::is_network_url_candidate(&path)
                {
                    path
                } else {
                    crate::platform::resolve_alias_path(&path)
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    riina_core::file_selection::prepare_cancellable(
        paths,
        open_media,
        crate::platform::natural_compare,
        cancelled,
    )
    .map_err(|error| error.to_string())
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
