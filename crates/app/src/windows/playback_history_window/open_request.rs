//! Recheck historical references without blocking the outline or its window owner.
use super::outline::path_from_reference;
pub(super) enum Intent {
    Single { position: Option<f64> },
    Play,
    PlayInNewWindow,
}

pub(super) struct Prepared {
    pub(super) paths: Vec<String>,
    pub(super) missing_local_file: bool,
}

pub(super) fn prepare(references: Vec<String>, cancelled: impl Fn() -> bool) -> Option<Prepared> {
    let mut prepared = Prepared {
        paths: Vec::with_capacity(references.len()),
        missing_local_file: false,
    };
    for reference in references {
        if cancelled() {
            return None;
        }
        if let Some(path) = path_from_reference(&reference) {
            if path.is_file() {
                if let Ok(path) = path.into_os_string().into_string() {
                    prepared.paths.push(path);
                }
            } else {
                prepared.missing_local_file = true;
            }
        } else if !reference.contains('\0')
            && url::Url::parse(&reference).is_ok_and(|url| url.scheme() != "file")
        {
            prepared.paths.push(reference);
        }
    }
    (!cancelled()).then_some(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_decodes_files_and_skips_removed_rows_without_reordering_references() {
        let tree = riina_test_support::TempDir::new("history-open-request").unwrap();
        let local = tree.path().join("episode #6.avi");
        std::fs::write(&local, []).unwrap();
        let local_url = url::Url::from_file_path(&local).unwrap().to_string();
        let gone = tree
            .path()
            .join("gone.avi")
            .into_os_string()
            .into_string()
            .unwrap();
        let remote = "https://media.example/episode%207.avi?list=two";
        let refs = vec![remote.into(), gone, local_url];

        assert!(prepare(refs.clone(), || true).is_none());
        let prepared = prepare(refs, || false).unwrap();
        assert!(prepared.missing_local_file);
        assert_eq!(prepared.paths, [remote, local.to_str().unwrap()]);
    }
}
