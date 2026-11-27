//! Resolve selected references before shared media discovery.
mod internet_shortcut;

pub fn prepare(paths: Vec<String>, open_media: bool) -> Result<Vec<String>, String> {
    prepare_cancellable(paths, open_media, || false)
}

pub(crate) fn prepare_cancellable(
    paths: Vec<String>,
    open_media: bool,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<String>, String> {
    prepare_with(paths, open_media, cancelled, resolve_reference)
}

pub(crate) fn prepare_plugin_sources(
    paths: Vec<String>,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<String>, String> {
    // The plugin's reviewed file remains the playback reference.
    prepare_with(paths, true, cancelled, |path| Ok(resolve_alias(path)))
}

fn prepare_with(
    paths: Vec<String>,
    open_media: bool,
    cancelled: impl Fn() -> bool,
    resolve: impl Fn(String) -> Result<String, String>,
) -> Result<Vec<String>, String> {
    let paths = paths
        .into_iter()
        .map(|path| {
            if cancelled() {
                return Err("Media selection was cancelled".to_owned());
            }
            resolve(path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    prepare_resolved(paths, open_media, cancelled)
}

pub(crate) fn resolve_reference(path: String) -> Result<String, String> {
    if riina_core::media_reference::is_network_url_candidate(&path) {
        return riina_core::stream_address::stream_address(&path)
            .map(|address| address.url.into_owned())
            .map_err(|error| error.to_string());
    }
    internet_shortcut::resolve(resolve_alias(path))
}

pub(crate) fn is_shortcut_reference(path: &str) -> bool {
    is_local_reference(path)
        && (internet_shortcut::is_path(path)
            || (cfg!(windows)
                && std::path::Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))))
}

fn resolve_alias(path: String) -> String {
    if is_local_reference(&path) {
        crate::platform::resolve_alias_path(&path)
    } else {
        path
    }
}

fn is_local_reference(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\0')
        && !riina_core::media_reference::is_network_url_candidate(path)
}

pub(crate) fn prepare_resolved(
    paths: Vec<String>,
    open_media: bool,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<String>, String> {
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

    #[test]
    fn only_user_selections_resolve_internet_shortcuts_before_discovery() {
        let directory = riina_test_support::TempDir::new("internet-shortcut-selection").unwrap();
        let shortcut = directory.path().join("Séance.URL");
        let contents =
            "[InternetShortcut]\r\nURL=https://example.test/片 {50%}.mkv?sig=A%2fb+Z\r\n";
        std::fs::write(
            &shortcut,
            std::iter::once(0xfeff)
                .chain(contents.encode_utf16())
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let path = shortcut.into_os_string().into_string().unwrap();
        let selected = || vec![path.clone()];
        for open in [true, false] {
            assert_eq!(
                super::prepare(selected(), open).unwrap(),
                ["https://example.test/%E7%89%87%20%7B50%25%7D.mkv?sig=A%2fb+Z"]
            );
        }
        assert_eq!(
            super::prepare_plugin_sources(selected(), || false).unwrap(),
            std::slice::from_ref(&path)
        );
        assert!(super::prepare_cancellable(selected(), true, || true).is_err());
    }
}
