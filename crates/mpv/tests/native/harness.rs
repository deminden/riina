use super::*;
use riina_mpv::{EngineError, StartupOptions};
use riina_test_support::TempDir;

pub(super) enum CaptureOutput {
    Temporary(TempDir),
    Retained(PathBuf),
}

impl CaptureOutput {
    pub(super) fn new(label: &str) -> std::io::Result<Self> {
        let Some(root) = std::env::var_os("RIINA_TEST_ARTIFACT_DIR") else {
            return TempDir::new(label).map(Self::Temporary);
        };
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "RIINA_TEST_ARTIFACT_DIR must be an absolute directory",
            ));
        }
        let path = root.join(label);
        // A new directory keeps every manifest tied to a single run.
        std::fs::create_dir(&path)?;
        Ok(Self::Retained(path))
    }

    pub(super) fn path(&self) -> &std::path::Path {
        match self {
            Self::Temporary(directory) => directory.path(),
            Self::Retained(path) => path,
        }
    }
}

pub(super) struct Harness {
    pub(super) player: Player,
    pub(super) receiver: mpsc::Receiver<Playback>,
}

impl Harness {
    pub(super) fn start(root: &std::path::Path) -> Result<Self, EngineError> {
        Self::with_options(root, Vec::new())
    }

    pub(super) fn with_options(
        root: &std::path::Path,
        options: impl Into<StartupOptions>,
    ) -> Result<Self, EngineError> {
        Self::at(&test_library(root), options)
    }

    pub(super) fn at(
        library: &std::path::Path,
        options: impl Into<StartupOptions>,
    ) -> Result<Self, EngineError> {
        let (sender, receiver) = mpsc::channel();
        let player = start_test_player_with_options(library, true, options, move |state| {
            let _ = sender.send(state);
        })?;
        Ok(Self { player, receiver })
    }

    #[track_caller]
    pub(super) fn wait(&self, predicate: impl Fn(&Playback) -> bool) -> Playback {
        wait_for(&self.receiver, predicate)
    }

    pub(super) fn send(&self, command: Command) -> Result<(), EngineError> {
        self.player.send(command)
    }
}

#[expect(
    clippy::expect_used,
    reason = "A missing fixture root must fail the native test"
)]
pub(super) fn fixture_root() -> PathBuf {
    riina_test_support::workspace_root()
        .expect("Native test fixtures must exist in the repository root")
}

#[expect(
    clippy::expect_used,
    reason = "A missing configured library must fail the native test"
)]
#[cfg_attr(
    not(target_os = "windows"),
    expect(
        clippy::panic,
        reason = "Other targets require an explicit library for native tests"
    )
)]
pub(super) fn test_library(root: &std::path::Path) -> PathBuf {
    if let Some(path) = std::env::var_os("RIINA_TEST_MPV_LIBRARY") {
        let path = PathBuf::from(path);
        assert!(
            path.is_absolute(),
            "The selected test library must have an absolute path"
        );
        return path
            .canonicalize()
            .expect("The selected test library must exist");
    }
    #[cfg(target_os = "windows")]
    return root.join("tools/mpv/libmpv-2.dll");
    #[cfg(not(target_os = "windows"))]
    {
        let _ = root;
        panic!("Set RIINA_TEST_MPV_LIBRARY to the absolute path of a provisioned libmpv library");
    }
}

pub(super) fn start_test_player(
    path: &std::path::Path,
    headless: bool,
    notify: impl Fn(Playback) + Send + 'static,
) -> Result<Player, EngineError> {
    start_test_player_with_options(path, headless, Vec::new(), notify)
}

pub(super) fn start_test_player_with_options(
    path: &std::path::Path,
    headless: bool,
    options: impl Into<StartupOptions>,
    notify: impl Fn(Playback) + Send + 'static,
) -> Result<Player, EngineError> {
    // Hosted runners have no physical endpoint. An explicit test AO preserves
    // decoding and clocked transport without changing the application's AO.
    let mut options = options.into();
    if let Some(output) = std::env::var_os("RIINA_TEST_MPV_AUDIO_OUTPUT") {
        let output = output.to_string_lossy().into_owned();
        if let Some((_, configured_output)) = options.mpv.iter_mut().find(|(key, _)| key == "ao") {
            *configured_output = output;
        } else {
            options.mpv.push(("ao".into(), output));
        }
    }
    Player::start_with_options(path, headless, options, move |state| {
        notify(std::sync::Arc::unwrap_or_clone(state))
    })
}

pub(super) fn selected_track(state: &Playback, kind: TrackKind) -> Option<i64> {
    state
        .tracks
        .iter()
        .find(|t| t.kind == kind && t.is_primary_selected())
        .map(|t| t.id)
}

#[track_caller]
pub(super) fn wait_for(
    receiver: &mpsc::Receiver<Playback>,
    predicate: impl Fn(&Playback) -> bool,
) -> Playback {
    riina_test_support::wait_for(receiver, Duration::from_secs(12), predicate)
}
