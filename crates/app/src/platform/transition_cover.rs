//! The native video owner holds the outgoing frame until the incoming generation settles.
use crate::transition_frame::{self, CoverRelease};
use riina_core::Playback;
use std::sync::atomic::AtomicU64;

pub(crate) fn should_display(
    state: &Playback,
    playlist_auto_play_next: bool,
    release_generation: &AtomicU64,
) -> bool {
    let release = if cfg!(target_os = "windows") {
        let stable_generation = match release_generation.load(std::sync::atomic::Ordering::Acquire)
        {
            u64::MAX => None,
            generation => Some(generation),
        };
        CoverRelease::NativeSurface { stable_generation }
    } else {
        CoverRelease::CapturedFrame
    };
    transition_frame::should_display(state, playlist_auto_play_next, release)
}
