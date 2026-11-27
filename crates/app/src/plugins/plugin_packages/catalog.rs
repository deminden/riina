//! Keep Settings and both plugin hosts on the same installed inventory.
use riina_core::plugin::Manifest;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct InstalledPlugin {
    pub manifest: Manifest,
    pub directory: PathBuf,
    pub(crate) declared_identifier: String,
    development: bool,
}

impl InstalledPlugin {
    pub(crate) fn is_development(&self) -> bool {
        self.development
    }

    pub(crate) fn can_update(&self) -> bool {
        !self.development
            && self.manifest.identifier == self.declared_identifier
            && self.directory.file_name().and_then(|name| name.to_str())
                == Some(&format!("{}.iinaplugin", self.declared_identifier))
    }
}

#[derive(Default)]
pub(crate) struct Catalog {
    pub plugins: Vec<InstalledPlugin>,
    pub invalid: usize,
}

pub(crate) fn discover(directory: &Path, locale: Option<&str>) -> Catalog {
    let mut catalog = Catalog::default();
    let Ok(root) = directory.canonicalize() else {
        return catalog;
    };
    for entry in fs::read_dir(directory).into_iter().flatten().flatten() {
        if entry.file_name().as_encoded_bytes().starts_with(b".") {
            continue;
        }
        let path = entry.path();
        let development = match path.extension().and_then(|extension| extension.to_str()) {
            Some("iinaplugin-dev") => true,
            Some("iinaplugin") => {
                path.canonicalize().ok().as_deref() != Some(&root.join(entry.file_name()))
            }
            _ => continue,
        };
        match Manifest::load_from_directory_with_locale(&path, locale) {
            Ok(manifest) => catalog.plugins.push(InstalledPlugin {
                declared_identifier: manifest.identifier.clone(),
                manifest,
                directory: path,
                development,
            }),
            Err(_) => catalog.invalid += 1,
        }
    }
    catalog.plugins.sort_by(|left, right| {
        left.development
            .cmp(&right.development)
            .then_with(|| left.directory.cmp(&right.directory))
    });
    let mut reserved = catalog
        .plugins
        .iter()
        .map(|plugin| plugin.declared_identifier.clone())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for plugin in &mut catalog.plugins {
        if seen.insert(plugin.declared_identifier.clone()) {
            continue;
        }
        // Keep a second copy's grants, settings and runtime separate. The
        // installed filename is stable across refreshes and player lifetimes.
        let name = plugin.directory.file_name().unwrap_or_default();
        let digest = crate::hashing::hex(&Sha256::digest(name.as_encoded_bytes()));
        let mut identifier = format!("{}.duplicate-{digest}", plugin.declared_identifier);
        while !reserved.insert(identifier.clone()) {
            identifier.push('x');
        }
        plugin.manifest.identifier = identifier;
    }
    catalog
        .plugins
        .sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    catalog
}
