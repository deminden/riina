//! Select and verify the exact Windows Setup asset for a release.
use crate::{
    http::{RequestPolicy, agent_config},
    platform::windows::update_setup,
};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::os::windows::fs::OpenOptionsExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use url::Url;
use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

const MAX_BYTES: u64 = 512 * 1024 * 1024;
const DOWNLOAD_TIME: Duration = Duration::from_secs(120);

#[derive(Default, Deserialize)]
#[serde(default)]
pub(crate) struct Asset {
    name: String,
    state: Option<String>,
    size: u64,
    digest: Option<String>,
}

#[derive(Clone)]
pub(crate) struct Package {
    name: String,
    address: String,
    size: u64,
    digest: [u8; 32],
}

pub(crate) struct Downloaded {
    path: PathBuf,
    package: Package,
    _directory: tempfile::TempDir,
}

impl Package {
    pub(crate) fn select(assets: Vec<Asset>, tag: &str, version: &Version) -> Option<Self> {
        if !cfg!(target_arch = "x86_64") {
            return None;
        }
        let name = format!("riina-{version}-windows-x86_64-setup.exe");
        let mut candidates = assets.into_iter().filter(|asset| asset.name == name);
        let asset = candidates.next()?;
        if candidates.next().is_some()
            || asset.state.as_deref() != Some("uploaded")
            || !(1..=MAX_BYTES).contains(&asset.size)
        {
            return None;
        }
        let digest = decode_digest(asset.digest.as_deref()?)?;
        Some(Self {
            address: format!("https://github.com/deminden/riina/releases/download/{tag}/{name}"),
            name,
            size: asset.size,
            digest,
        })
    }

    pub(crate) fn download(self, cancelled: &AtomicBool) -> Result<Downloaded, String> {
        let deadline = Instant::now() + DOWNLOAD_TIME;
        let mut address = Url::parse(&self.address).map_err(|error| error.to_string())?;
        for _ in 0..=3 {
            check_cancelled(cancelled).map_err(|error| error.to_string())?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("The installer download timed out.".into());
            }
            let agent = agent_config(remaining, RequestPolicy::ManualRedirects).new_agent();
            let mut response = agent
                .get(address.as_str())
                .call()
                .map_err(|_| "Could not download the installer from GitHub.")?;
            match response.status().as_u16() {
                200 => {
                    return self
                        .stage(response.body_mut().as_reader(), cancelled)
                        .map_err(|error| error.to_string());
                }
                301 | 302 | 303 | 307 | 308 => {
                    address = response
                        .headers()
                        .get("location")
                        .and_then(|location| location.to_str().ok())
                        .and_then(|location| download_redirect(&address, location))
                        .ok_or("GitHub returned an unexpected installer download address.")?;
                }
                code => return Err(format!("Installer download failed with HTTP {code}.")),
            }
        }
        Err("GitHub redirected the installer download too many times.".into())
    }

    fn stage(self, reader: impl Read, cancelled: &AtomicBool) -> io::Result<Downloaded> {
        let directory = tempfile::Builder::new().prefix("riina-update-").tempdir()?;
        let pending = directory.path().join("installer.download");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        self.copy_verified(reader, &mut file, cancelled)?;
        file.sync_all()?;
        drop(file);
        check_cancelled(cancelled)?;

        let path = directory.path().join(&self.name);
        fs::rename(pending, &path)?;
        update_setup::mark_download(&path, &self.address)?;
        Ok(Downloaded {
            path,
            package: self,
            _directory: directory,
        })
    }

    fn copy_verified(
        &self,
        reader: impl Read,
        mut target: impl Write,
        cancelled: &AtomicBool,
    ) -> io::Result<()> {
        let mut reader = reader.take(self.size + 1);
        let mut hash = Sha256::new();
        let mut received = 0;
        let mut buffer = [0; 32 * 1024];
        loop {
            check_cancelled(cancelled)?;
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if received > self.size {
                return Err(io::Error::other(
                    "Installer size differs from the published release.",
                ));
            }
            hash.update(&buffer[..count]);
            target.write_all(&buffer[..count])?;
        }
        if received != self.size {
            return Err(io::Error::other(
                "Installer size differs from the published release.",
            ));
        }
        let actual: [u8; 32] = hash.finalize().into();
        if actual != self.digest {
            return Err(io::Error::other(
                "Installer checksum differs from the published release.",
            ));
        }
        Ok(())
    }
}

impl Downloaded {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Keep writers and delete/rename excluded from verification until Windows opens Setup.
    pub(crate) fn lock_verified(&self) -> io::Result<File> {
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(&self.path)?;
        self.package
            .copy_verified(&mut file, io::sink(), &AtomicBool::new(false))?;
        Ok(file)
    }
}

fn check_cancelled(cancelled: &AtomicBool) -> io::Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Installer download cancelled.",
        ));
    }
    Ok(())
}

fn download_redirect(from: &Url, target: &str) -> Option<Url> {
    let url = from.join(target).ok()?;
    (url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com")
        )
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none())
    .then_some(url)
}

fn decode_digest(value: &str) -> Option<[u8; 32]> {
    let hex = value.strip_prefix("sha256:")?;
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&hex[start..start + 2], 16).ok()?;
    }
    Some(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(data: &[u8]) -> Package {
        let digest = crate::hashing::hex(&Sha256::digest(data));
        Package::select(
            vec![Asset {
                name: "riina-5.0.0-beta.2-windows-x86_64-setup.exe".into(),
                state: Some("uploaded".into()),
                size: data.len() as u64,
                digest: Some(format!("sha256:{digest}")),
            }],
            "v5.0.0-beta.2",
            &Version::parse("5.0.0-beta.2").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn verifies_and_locks_the_installer_through_handoff() -> io::Result<()> {
        let data = b"verified installer";
        let package = fixture(data);
        assert!(
            package
                .address
                .ends_with("/v5.0.0-beta.2/riina-5.0.0-beta.2-windows-x86_64-setup.exe")
        );
        let installer = package.stage(&data[..], &AtomicBool::new(false))?;
        let stream = format!("{}:Zone.Identifier", installer.path().display());
        assert!(fs::read_to_string(stream)?.contains("ZoneId=3"));
        let lock = installer.lock_verified()?;
        assert!(fs::write(installer.path(), b"replaced installer").is_err());
        drop(lock);
        fs::write(installer.path(), b"replaced installer")?;
        assert!(installer.lock_verified().is_err());
        let path = installer.path().to_path_buf();
        drop(installer);
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn rejects_corrupt_incomplete_oversized_and_cancelled_transfers() {
        let package = fixture(b"correct");
        let active = AtomicBool::new(false);
        for data in [b"short".as_slice(), b"corrupt", b"correct with extra data"] {
            assert!(package.copy_verified(data, io::sink(), &active).is_err());
        }
        active.store(true, Ordering::Relaxed);
        let cancelled = package.copy_verified(b"correct".as_slice(), io::sink(), &active);
        assert_eq!(cancelled.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn release_packages_require_unique_uploaded_assets_and_github_cdn() {
        let version = Version::new(5, 0, 0);
        let asset = |state: &str| Asset {
            name: "riina-5.0.0-windows-x86_64-setup.exe".into(),
            state: Some(state.into()),
            size: 512,
            digest: Some(format!("sha256:{}", "a".repeat(64))),
        };
        assert!(Package::select(vec![asset("new")], "v5.0.0", &version).is_none());
        assert!(
            Package::select(
                vec![asset("uploaded"), asset("uploaded")],
                "v5.0.0",
                &version
            )
            .is_none()
        );
        let from = Url::parse(&fixture(b"correct").address).unwrap();
        for address in [
            "http://release-assets.githubusercontent.com/pkg",
            "https://release-assets.githubusercontent.com.example.com/pkg",
            "https://user@release-assets.githubusercontent.com/pkg",
            "https://release-assets.githubusercontent.com:8443/pkg",
            "https://github.com/another/project/releases/download/executable",
        ] {
            assert!(download_redirect(&from, address).is_none(), "{address}");
        }
        assert!(
            download_redirect(
                &from,
                "https://release-assets.githubusercontent.com/pkg?token=123"
            )
            .is_some()
        );
    }
}
