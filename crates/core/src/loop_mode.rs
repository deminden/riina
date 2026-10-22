//! File and playlist repeat policy.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Off,
    File,
    Playlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DefaultRepeatMode {
    #[default]
    Playlist,
    File,
}

impl DefaultRepeatMode {
    pub const fn loop_mode(self) -> LoopMode {
        match self {
            Self::Playlist => LoopMode::Playlist,
            Self::File => LoopMode::File,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Playlist),
            1 => Some(Self::File),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Playlist => 0,
            Self::File => 1,
        }
    }
}
impl LoopMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::File,
            Self::File => Self::Playlist,
            Self::Playlist => Self::Off,
        }
    }
}
