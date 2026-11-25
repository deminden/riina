//! Apply Pause When Open before libmpv automatically starts the next entry.
use super::{Handle, WorkerState};
use crate::{ffi::end_reason, pending::ReplyKind, record_notice};
use riina_core::NoticeSeverity;

pub(super) const HOOK_SLOT: u64 = u64::MAX - 1;

#[derive(Default)]
pub(super) struct EndPause {
    installed: bool,
    armed: bool,
    command_and_hook: Option<(u64, u64)>,
}

impl EndPause {
    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl WorkerState {
    pub(super) fn install_end_pause_hook(&mut self, handle: &Handle) {
        if self.playlist_end_pause.installed {
            return;
        }
        match handle.add_hook(HOOK_SLOT, c"on_after_end_file", -10) {
            Ok(()) => self.playlist_end_pause.installed = true,
            Err(error) => {
                record_notice(
                    &mut self.playback,
                    NoticeSeverity::Warning,
                    error.to_string(),
                );
            }
        }
    }

    pub(super) fn arm_end_pause(&mut self, reason: i32) {
        // STOP also covers manual navigation and replacement. Those commands
        // already specify their own pause policy, as do redirected opens.
        self.playlist_end_pause.armed = self.playlist_end_pause.installed
            && matches!(reason, end_reason::EOF | end_reason::ERROR)
            && self.playback.pause_when_open
            && !self.pending.values().any(|command| command.changes_media);
    }

    pub(super) fn pause_after_end(&mut self, handle: &Handle, hook: u64) {
        if std::mem::take(&mut self.playlist_end_pause.armed) && self.playback.pause_when_open {
            let args = ["set".into(), "pause".into(), "yes".into()];
            match self.issue(handle, &args, ReplyKind::Plain) {
                Ok(()) => {
                    // An END_FILE notification alone can arrive too late for
                    // a short audio file. mpv waits here for our command reply.
                    self.playlist_end_pause.command_and_hook = Some((self.next_id, hook));
                    return;
                }
                Err(error) => record_notice(&mut self.playback, NoticeSeverity::Error, error),
            }
        }
        handle.continue_hook(hook);
    }

    pub(super) fn finish_end_pause(&mut self, handle: &Handle, command: u64) {
        if self
            .playlist_end_pause
            .command_and_hook
            .is_some_and(|(id, _)| id == command)
        {
            self.release_end_pause_hook(handle);
        }
    }

    pub(super) fn release_end_pause_hook(&mut self, handle: &Handle) {
        self.playlist_end_pause.disarm();
        if let Some((_, hook)) = self.playlist_end_pause.command_and_hook.take() {
            handle.continue_hook(hook);
        }
    }
}
