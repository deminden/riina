//! User profile storage stays outside shared playback state and UI widgets.

use riina_core::key_bindings::{self, Mapping, ParseError};
use std::{
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum ProfileError {
    Io(std::io::Error),
    Parse(ParseError),
    InvalidName,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Parse(error) => error.fmt(formatter),
            Self::InvalidName => formatter.write_str("Invalid profile name"),
        }
    }
}

impl std::error::Error for ProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::InvalidName => None,
        }
    }
}

impl From<std::io::Error> for ProfileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ParseError> for ProfileError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProfileStore {
    directory: PathBuf,
}

impl ProfileStore {
    pub fn for_current_user() -> Result<Self, ProfileError> {
        let directory = dirs::config_dir()
            .ok_or_else(|| {
                std::io::Error::other("The user configuration directory is unavailable")
            })?
            .join("riina")
            .join("input");
        Self::at(directory)
    }

    fn at(directory: PathBuf) -> Result<Self, ProfileError> {
        fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn list(&self) -> Result<Vec<Profile>, ProfileError> {
        let mut profiles = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() || path.extension() != Some(OsStr::new("conf")) {
                continue;
            }
            let Some(name) = path.file_stem().and_then(OsStr::to_str) else {
                continue;
            };
            profiles.push(Profile {
                name: name.to_owned(),
                path,
            });
        }
        profiles.sort_by_cached_key(|profile| profile.name.to_lowercase());
        Ok(profiles)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn path_for(&self, name: &str) -> Result<PathBuf, ProfileError> {
        self.profile_path(name)
    }

    pub fn load(&self, name: &str) -> Result<Vec<Mapping>, ProfileError> {
        let path = self.profile_path(name)?;
        reject_symlink(&path)?;
        Ok(key_bindings::parse(&fs::read_to_string(path)?)?)
    }

    pub fn create(&self, name: &str) -> Result<(), ProfileError> {
        let path = self.profile_path(name)?;
        let contents = key_bindings::serialize(&[])?;
        write_new(&path, contents.as_bytes())?;
        Ok(())
    }

    pub fn duplicate(&self, source: &str, target: &str) -> Result<(), ProfileError> {
        let mappings = self.load(source)?;
        let path = self.profile_path(target)?;
        write_new(&path, key_bindings::serialize(&mappings)?.as_bytes())?;
        Ok(())
    }

    pub fn import(&self, source: &Path) -> Result<String, ProfileError> {
        if source.extension() != Some(OsStr::new("conf")) {
            return Err(ProfileError::InvalidName);
        }
        let name = source
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or(ProfileError::InvalidName)?;
        let mappings = key_bindings::parse(&fs::read_to_string(source)?)?;
        let path = self.profile_path(name)?;
        write_new(&path, key_bindings::serialize(&mappings)?.as_bytes())?;
        Ok(name.to_owned())
    }

    pub fn save(&self, name: &str, mappings: &[Mapping]) -> Result<(), ProfileError> {
        let path = self.profile_path(name)?;
        reject_symlink(&path)?;
        if !path.is_file() {
            return Err(
                std::io::Error::new(std::io::ErrorKind::NotFound, "Profile not found").into(),
            );
        }
        crate::platform::settings_file::save(&path, &key_bindings::serialize(mappings)?)?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<(), ProfileError> {
        let path = self.profile_path(name)?;
        reject_symlink(&path)?;
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn rename(&self, current: &str, replacement: &str) -> Result<(), ProfileError> {
        let source = self.profile_path(current)?;
        let target = self.profile_path(replacement)?;
        if source == target {
            return Ok(());
        }
        reject_symlink(&source)?;
        if !source.is_file() {
            return Err(
                std::io::Error::new(std::io::ErrorKind::NotFound, "Profile not found").into(),
            );
        }
        let mappings = self.load(current)?;
        write_new(&target, key_bindings::serialize(&mappings)?.as_bytes())?;
        if let Err(error) = fs::remove_file(source) {
            let _ = fs::remove_file(target);
            return Err(error.into());
        }
        Ok(())
    }

    fn profile_path(&self, name: &str) -> Result<PathBuf, ProfileError> {
        validate_name(name)?;
        Ok(self.directory.join(format!("{name}.conf")))
    }
}

fn validate_name(name: &str) -> Result<(), ProfileError> {
    if name.is_empty()
        || name.trim() != name
        || name.len() > 120
        || name.ends_with('.')
        || name.ends_with(' ')
        || name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        || [
            "IINA Default",
            "mpv Default",
            "VLC Default",
            "Movist Default",
            "Movist v2 Default",
        ]
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(name))
    {
        return Err(ProfileError::InvalidName);
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), ProfileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ProfileError::InvalidName),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn write_new(path: &Path, contents: &[u8]) -> Result<(), ProfileError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "riina-key-profiles-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn user_profiles_create_duplicate_edit_import_rename_and_delete_safely() {
        let fixture = Fixture::new();
        let store = ProfileStore::at(fixture.0.join("profiles")).unwrap();
        store.create("Cinema").unwrap();
        store
            .save(
                "Cinema",
                &[Mapping {
                    key: "SPACE".into(),
                    action: "cycle pause".into(),
                    kind: key_bindings::ActionKind::Mpv,
                    comment: None,
                }],
            )
            .unwrap();
        store.duplicate("Cinema", "Cinema copy").unwrap();
        assert_eq!(
            store.load("Cinema copy").unwrap(),
            store.load("Cinema").unwrap()
        );
        assert_eq!(store.list().unwrap().len(), 2);

        let import = fixture.0.join("imported.conf");
        fs::write(&import, "LEFT seek -5\n").unwrap();
        store.import(&import).unwrap();
        assert_eq!(store.load("imported").unwrap()[0].action, "seek -5");
        store.rename("imported", "Imported profile").unwrap();
        assert!(store.load("imported").is_err());
        assert_eq!(store.load("Imported profile").unwrap()[0].action, "seek -5");
        store.delete("Cinema copy").unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn invalid_or_builtin_names_cannot_be_written() {
        let fixture = Fixture::new();
        let profiles = fixture.0.join("profiles");
        let store = ProfileStore::at(profiles.clone()).unwrap();
        for name in ["", "..", "../escape", "IINA Default", "trailing."] {
            assert!(store.create(name).is_err(), "{name:?}");
        }
        assert_eq!(fs::read_dir(&profiles).unwrap().count(), 0);
    }

    #[test]
    fn malformed_import_does_not_create_a_profile() {
        let fixture = Fixture::new();
        let profiles = fixture.0.join("profiles");
        let store = ProfileStore::at(profiles.clone()).unwrap();
        let import = fixture.0.join("broken.conf");
        fs::write(&import, "SPACE\n").unwrap();
        assert!(store.import(&import).is_err());
        assert_eq!(store.list().unwrap().len(), 0);
        assert_eq!(fs::read_dir(profiles).unwrap().count(), 0);
    }
}
