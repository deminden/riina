use super::*;

#[test]
fn stages_complete_trees_and_preserves_existing_destinations() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let source = root.join("source");
    let destination = root.join("destination");
    fs::create_dir_all(source.join("nested/empty")).unwrap();
    fs::write(source.join("nested/épisode.dat"), [0, 42, 255]).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("original"), "keep").unwrap();

    let error = move_new(&source, &destination).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(destination.join("original")).unwrap(), b"keep");
    assert!(source.join("nested/empty").is_dir());

    let destination = root.join("moved");
    move_new(&source, &destination).unwrap();
    assert!(!source.exists());
    assert_eq!(
        fs::read(destination.join("nested/épisode.dat")).unwrap(),
        [0, 42, 255]
    );
    assert!(destination.join("nested/empty").is_dir());
    assert_eq!(fs::read_dir(root).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn keeps_links_as_links_and_rolls_back_for_special_entries() {
    use std::os::unix::{fs::symlink, net::UnixListener};

    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source");
    let destination = fixture.path().join("destination");
    fs::create_dir(&source).unwrap();
    symlink("missing-target", source.join("link")).unwrap();
    let socket = source.join("active.sock");
    let listener = UnixListener::bind(&socket).unwrap();

    let error = move_new(&source, &destination).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert!(!destination.exists());
    assert!(socket.exists());
    drop(listener);
    fs::remove_file(socket).unwrap();

    move_new(&source, &destination).unwrap();
    assert_eq!(
        fs::read_link(destination.join("link")).unwrap(),
        Path::new("missing-target")
    );
    assert!(!source.exists());
}
