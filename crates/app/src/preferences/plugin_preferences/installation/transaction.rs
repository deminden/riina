//! Place reviewed packages and restore the previous files and profile on failure.
use super::super::{
    current_runtime_supports,
    inventory::{read_enabled, set_enabled},
    permissions::{read_plugin_grants, restore_plugin_grants, save_granted_permissions},
    updates::append_recovery,
};
use crate::{
    plugin_host::GlobalHandle,
    plugin_packages::{InstalledPlugin, StagedPackage, discover},
};
use riina_core::plugin::Manifest;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub(super) struct Profile {
    plugins: PathBuf,
    preferences: PathBuf,
    enabled: PathBuf,
    host: Option<GlobalHandle>,
}

impl Profile {
    pub(super) fn new(
        plugins: PathBuf,
        preferences: PathBuf,
        enabled: PathBuf,
        host: Option<GlobalHandle>,
    ) -> Self {
        Self {
            plugins,
            preferences,
            enabled,
            host,
        }
    }

    pub(super) fn plugins(&self) -> &Path {
        &self.plugins
    }

    pub(super) fn review(&self, staged: StagedPackage) -> Result<Review, String> {
        let previous = discover(&self.plugins, None)
            .plugins
            .into_iter()
            .find(|plugin| plugin.manifest.identifier == staged.manifest.identifier);
        if previous.as_ref().is_some_and(|plugin| !plugin.can_update()) {
            return Err("The existing plugin is managed outside the package installer.".into());
        }
        Ok(Review { staged, previous })
    }

    pub(super) fn install(&self, review: Review) -> Result<Installed, String> {
        let Review { staged, previous } = review;
        let plan = staged.reserve(&self.plugins, previous.as_ref())?;
        let manifest = plan.manifest().clone();
        let identifier = &manifest.identifier;
        let grants = read_plugin_grants(&self.preferences, identifier)?;
        let was_enabled = read_enabled(&self.enabled)?.contains(identifier);
        let enable = self.host.is_some() && current_runtime_supports(&manifest);
        let running = previous.as_ref().filter(|_| was_enabled);
        if running.is_some() && !enable {
            return Err(
                "The enabled plugin cannot be replaced by a package this build cannot run.".into(),
            );
        }
        if running.is_some()
            && let Some(host) = &self.host
            && let Err(error) = host.suspend_for_update(identifier.clone())
        {
            return Err(self.restart_previous(running, error));
        }

        let mut placed = plan
            .place()
            .map_err(|error| self.restart_previous(running, error))?;
        let mut activated = false;
        let committed = (|| {
            save_granted_permissions(&self.preferences, &manifest)?;
            set_enabled(&self.enabled, identifier, enable)?;
            if enable && let Some(host) = &self.host {
                activated = true;
                host.activate_after_update(manifest.clone(), placed.directory().to_owned())?;
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = committed {
            let mut recovery = Vec::new();
            if activated && let Some(host) = &self.host {
                observe(host.suspend_for_update(identifier.clone()), &mut recovery);
            }
            observe(set_enabled(&self.enabled, identifier, false), &mut recovery);
            observe(placed.rollback(), &mut recovery);
            observe(
                restore_plugin_grants(&self.preferences, identifier, grants.as_deref()),
                &mut recovery,
            );
            if recovery.is_empty() && was_enabled {
                observe(set_enabled(&self.enabled, identifier, true), &mut recovery);
            }
            let restored = recovery.is_empty();
            let error = append_recovery(format!("Could not install the plugin: {error}"), recovery);
            if activated
                && running.is_none()
                && let Some(host) = &self.host
            {
                let mut recovery = Vec::new();
                observe(host.disable(identifier.clone()), &mut recovery);
                return Err(append_recovery(error, recovery));
            }
            return Err(self.restart_previous(running.filter(|_| restored), error));
        }
        if let Err(error) = placed.commit() {
            crate::diagnostics::warn!("{error}");
        }
        Ok(Installed {
            manifest,
            replaced: previous.is_some(),
            enabled: enable,
        })
    }

    fn restart_previous(&self, running: Option<&InstalledPlugin>, error: String) -> String {
        let mut recovery = Vec::new();
        if let (Some(host), Some(plugin)) = (&self.host, running) {
            if Manifest::load_from_directory(&plugin.directory)
                .is_ok_and(|manifest| manifest == plugin.manifest)
            {
                observe(
                    host.activate_after_update(plugin.manifest.clone(), plugin.directory.clone()),
                    &mut recovery,
                );
            } else {
                recovery.push("Riina could not verify the previous plugin for restart".into());
            }
        }
        append_recovery(error, recovery)
    }
}

fn observe<T>(result: Result<T, String>, recovery: &mut Vec<String>) {
    if let Err(error) = result {
        recovery.push(error);
    }
}

pub(super) struct Review {
    staged: StagedPackage,
    previous: Option<InstalledPlugin>,
}

impl Review {
    pub(super) fn manifest(&self) -> &Manifest {
        &self.staged.manifest
    }

    pub(super) fn replaces(&self) -> bool {
        self.previous.is_some()
    }
}

pub(super) struct Installed {
    pub(super) manifest: Manifest,
    pub(super) replaced: bool,
    pub(super) enabled: bool,
}
