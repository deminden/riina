//! The native video owner holds the outgoing frame until the incoming generation settles.
use riina_core::Playback;
use std::sync::atomic::AtomicU64;

#[cfg(target_os = "windows")]
pub(crate) fn should_display(
    state: &Playback,
    playlist_auto_play_next: bool,
    release_generation: &AtomicU64,
) -> bool {
    crate::transition_frame::should_display_until_incoming_stable(
        state,
        playlist_auto_play_next,
        match release_generation.load(std::sync::atomic::Ordering::Acquire) {
            u64::MAX => None,
            generation => Some(generation),
        },
    )
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn should_display(
    state: &Playback,
    playlist_auto_play_next: bool,
    _release_generation: &AtomicU64,
) -> bool {
    crate::transition_frame::should_display(state, playlist_auto_play_next)
}
