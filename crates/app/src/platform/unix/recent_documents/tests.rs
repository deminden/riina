use super::*;
use riina_test_support::TempDir;
use std::{
    cell::Cell,
    fs,
    time::{Duration, SystemTime},
};

const SHARED: &std::ffi::CStr = c"file:///fixtures/shared%20caf%C3%A9.mkv";
const RIINA_ONLY: &std::ffi::CStr = c"file:///fixtures/private.mkv";
const OTHER: &std::ffi::CStr = c"Another Player";

fn seed(path: &Path) -> io::Result<()> {
    update(path, |bookmarks| {
        bookmarks.register(SHARED, c"video/x-matroska", OTHER, c"another-player %u");
        true
    })
}

#[test]
fn clearing_recents_preserves_other_desktop_applications() {
    let directory = TempDir::new("desktop-recents-ownership").unwrap();
    let path = directory.path().join(BOOKMARKS_FILE);
    seed(&path).unwrap();
    update(&path, |bookmarks| {
        bookmarks.register(RIINA_ONLY, c"video/x-matroska", APPLICATION, c"riina %u");
        bookmarks.register(
            SHARED,
            c"application/octet-stream",
            APPLICATION,
            c"riina %u",
        );
        true
    })
    .unwrap();
    let before = fs::read_to_string(&path).unwrap();
    assert!(before.contains(SHARED.to_str().unwrap()));
    assert!(before.contains(RIINA_ONLY.to_str().unwrap()));
    assert!(before.contains("application name=\"Riina\""));
    assert!(before.contains("video/x-matroska"));
    assert!(!before.contains("application/octet-stream"));

    update(&path, |bookmarks| bookmarks.remove_application(APPLICATION)).unwrap();
    let after = fs::read_to_string(&path).unwrap();
    assert!(after.contains(SHARED.to_str().unwrap()));
    assert!(after.contains("Another Player"));
    assert!(!after.contains(RIINA_ONLY.to_str().unwrap()));
    assert!(!after.contains("Riina"));
    update(&path, |bookmarks| bookmarks.remove_application(APPLICATION)).unwrap();
    assert_eq!(after, fs::read_to_string(&path).unwrap());
}

#[test]
fn malformed_and_concurrently_updated_desktop_recents_keep_their_data() {
    let directory = TempDir::new("desktop-recents-races").unwrap();
    let path = directory.path().join(BOOKMARKS_FILE);
    let replacement = directory.path().join("other-app.xbel");
    seed(&replacement).unwrap();
    update(&path, |bookmarks| {
        bookmarks.register(RIINA_ONLY, c"video/x-matroska", APPLICATION, c"riina %u");
        true
    })
    .unwrap();
    let calls = Cell::new(0);
    update(&path, |bookmarks| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            fs::copy(&replacement, &path).unwrap();
            fs::File::open(&path)
                .unwrap()
                .set_modified(SystemTime::now() + Duration::from_secs(10))
                .unwrap();
        }
        bookmarks.register(RIINA_ONLY, c"video/x-matroska", APPLICATION, c"riina %u");
        true
    })
    .unwrap();
    assert_eq!(calls.get(), 2);
    let written = fs::read_to_string(&path).unwrap();
    assert!(written.contains("Another Player"));
    assert!(written.contains("Riina"));

    fs::write(&path, b"<partial-xbel").unwrap();
    assert!(seed(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"<partial-xbel");
}
