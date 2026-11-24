//! Keep the application's update choices and attempt history across launches.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Permission {
    #[default]
    Undecided,
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Interval {
    Hourly,
    #[default]
    Daily,
    Weekly,
    Monthly,
}

impl Interval {
    pub(super) const ALL: [Self; 4] = [Self::Hourly, Self::Daily, Self::Weekly, Self::Monthly];

    fn duration(self) -> Duration {
        Duration::from_secs(match self {
            Self::Hourly => 3_600,
            Self::Daily => 86_400,
            Self::Weekly => 604_800,
            Self::Monthly => 2_629_800,
        })
    }

    pub(super) fn index(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Channel {
    #[default]
    Stable,
    Prereleases,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct State {
    pub(super) permission: Permission,
    pub(super) interval: Interval,
    pub(super) include_prereleases: bool,
    seen_launch: bool,
    stable_attempt: Option<u64>,
    prerelease_attempt: Option<u64>,
}

impl State {
    pub(super) fn should_ask(&mut self) -> bool {
        let has_launched = self.seen_launch;
        self.seen_launch = true;
        has_launched && self.permission == Permission::Undecided
    }

    pub(super) fn channel(self) -> Channel {
        if self.include_prereleases {
            Channel::Prereleases
        } else {
            Channel::Stable
        }
    }

    pub(super) fn wait(self, now: u64) -> Option<Duration> {
        if self.permission != Permission::Enabled {
            return None;
        }
        let last = match self.channel() {
            Channel::Stable => self.stable_attempt,
            Channel::Prereleases => self.prerelease_attempt,
        };
        // A clock correction must not defer checks indefinitely.
        let elapsed = last
            .filter(|last| *last <= now)
            .map_or(u64::MAX, |last| now - last);
        Some(
            self.interval
                .duration()
                .saturating_sub(Duration::from_secs(elapsed)),
        )
    }

    pub(super) fn mark_attempt(&mut self, channel: Channel, now: u64) {
        match channel {
            Channel::Stable => self.stable_attempt = Some(now),
            Channel::Prereleases => self.prerelease_attempt = Some(now),
        }
    }

    pub(super) fn claim_due(&mut self, now: u64) -> Option<Channel> {
        if !self.wait(now)?.is_zero() {
            return None;
        }
        let channel = self.channel();
        self.mark_attempt(channel, now);
        Some(channel)
    }
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) struct Store(PathBuf);

impl Store {
    pub(super) fn for_preferences(preferences_path: &Path) -> Self {
        Self(preferences_path.with_file_name("software-update.json"))
    }

    pub(super) fn read(&self) -> io::Result<State> {
        match fs::read(&self.0) {
            Ok(data) => serde_json::from_slice(&data).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(State::default()),
            Err(error) => Err(error),
        }
    }

    pub(super) fn change<T>(&self, edit: impl FnOnce(&mut State) -> T) -> io::Result<T> {
        if let Some(directory) = self.0.parent() {
            fs::create_dir_all(directory)?;
        }
        // Independent app processes share the same schedule and each update copies
        // the other process's current choices before applying a field change.
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.0.with_extension("json.lock"))?;
        lock.lock()?;
        let mut state = self.read()?;
        let before = state;
        let result = edit(&mut state);
        if before != state {
            crate::platform::save_preferences(&self.0, &serde_json::to_string_pretty(&state)?)?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_and_channel_schedules_survive_restart() -> io::Result<()> {
        let directory = riina_test_support::TempDir::new("update-schedule")?;
        let profile = directory.path().join("preferences.json");
        let store = Store::for_preferences(&profile);
        assert!(!store.change(State::should_ask)?);
        assert_eq!(store.read()?.wait(42), None);
        let restarted = Store::for_preferences(&profile);
        assert!(restarted.change(State::should_ask)?);
        restarted.change(|state| state.permission = Permission::Enabled)?;
        assert!(!restarted.change(State::should_ask)?);
        assert!(store.change(|state| state.claim_due(42))? == Some(Channel::Stable));
        assert!(restarted.change(|state| state.claim_due(43))?.is_none());
        assert_eq!(store.read()?.wait(43), Some(Duration::from_secs(86_399)));
        store.change(|state| {
            state.include_prereleases = true;
            state.interval = Interval::Hourly;
        })?;
        assert!(restarted.change(|state| state.claim_due(43))? == Some(Channel::Prereleases));
        assert_eq!(store.read()?.wait(44), Some(Duration::from_secs(3_599)));
        assert_eq!(store.read()?.wait(41), Some(Duration::ZERO));
        store.change(|state| state.permission = Permission::Disabled)?;
        assert_eq!(restarted.read()?.wait(1_000_000), None);
        Ok(())
    }
}
