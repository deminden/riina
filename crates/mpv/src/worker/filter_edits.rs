//! Order user edits against native filter-chain reads and replies.
use super::{EventRef, Handle, WorkerState};
use crate::{
    EngineError, MpvPropertyFormat, client_request::Operation, ffi::event_id, pending::ReplyKind,
    record_notice,
};
use riina_core::{Command, NoticeSeverity, Playback, crop::VideoFilter};
use std::{
    collections::VecDeque,
    ffi::CStr,
    time::{Duration, Instant},
};

const MAX_WAITING: usize = 64;
const REPLY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
enum Chain {
    Audio,
    Video,
}

impl Chain {
    fn property(self) -> &'static CStr {
        match self {
            Self::Audio => c"af",
            Self::Video => c"vf",
        }
    }

    fn planning_state(self, filters: Vec<VideoFilter>) -> Playback {
        match self {
            Self::Audio => Playback {
                audio_filters: Some(filters),
                ..Playback::default()
            },
            Self::Video => Playback {
                video_filters: Some(filters),
                ..Playback::default()
            },
        }
    }
}

pub(crate) struct Target {
    chain: Chain,
    generation: u64,
    feedback: Option<String>,
    expected: Option<Vec<VideoFilter>>,
}

impl Target {
    pub(crate) fn named(
        generation: u64,
        command: &Command,
        name: String,
    ) -> Result<Self, EngineError> {
        let chain = match command {
            Command::AddAudioFilter(_)
            | Command::ToggleAudioFilter(_)
            | Command::RemoveAudioFilter(_) => Chain::Audio,
            Command::AddVideoFilter(_)
            | Command::ToggleVideoFilter(_)
            | Command::RemoveVideoFilter(_) => Chain::Video,
            _ => {
                return Err(EngineError::InvalidArgument {
                    reason: "The command is not a user filter addition, removal or toggle",
                });
            }
        };
        Ok(Self {
            chain,
            generation,
            feedback: Some(name),
            expected: None,
        })
    }

    pub(crate) fn indexed(
        generation: u64,
        command: &Command,
        expected: Vec<VideoFilter>,
    ) -> Result<Self, EngineError> {
        let (chain, removing) = match command {
            Command::RemoveAudioFilterAt(_) => (Chain::Audio, true),
            Command::RemoveVideoFilterAt(_) => (Chain::Video, true),
            Command::SetAudioFilterAt { .. } => (Chain::Audio, false),
            Command::SetVideoFilterAt { .. } => (Chain::Video, false),
            _ => {
                return Err(EngineError::InvalidArgument {
                    reason: "The command is not an indexed filter removal or replacement",
                });
            }
        };
        Ok(Self {
            chain,
            generation,
            feedback: removing.then(String::new),
            expected: Some(expected),
        })
    }
}

pub(crate) struct Request {
    pub(crate) command: Command,
    pub(crate) target: Target,
}

pub(super) struct ReadResult {
    request: Request,
    filters: Result<Vec<VideoFilter>, String>,
}

enum InFlight {
    Read {
        id: u64,
        deadline: Instant,
        request: Request,
    },
    Edit {
        id: u64,
        deadline: Instant,
    },
}

#[derive(Default)]
pub(super) struct Queue {
    waiting: VecDeque<Request>,
    active: Option<InFlight>,
}

impl Queue {
    pub(super) fn deadline(&self) -> Option<Instant> {
        match self.active.as_ref()? {
            InFlight::Read { deadline, .. } | InFlight::Edit { deadline, .. } => Some(*deadline),
        }
    }

    pub(super) fn edit_replied(&mut self, id: u64) {
        if matches!(self.active, Some(InFlight::Edit { id: active, .. }) if active == id) {
            self.active = None;
        }
    }
}

impl WorkerState {
    pub(super) fn enqueue_filter_edit(&mut self, request: Request) {
        if self.filter_edits.waiting.len() >= MAX_WAITING {
            self.filter_edit_error("Too many filter edits are queued. Try again.".into());
        } else {
            self.filter_edits.waiting.push_back(request);
        }
    }

    pub(super) fn advance_filter_edits(&mut self, handle: &Handle) {
        if self
            .filter_edits
            .deadline()
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.retire_filter_edits();
            self.filter_edit_error("The engine did not confirm the filter edit. Check the active filters before trying again.".into());
        }
        if self.filter_edits.active.is_some() {
            return;
        }
        while let Some(request) = self.filter_edits.waiting.pop_front() {
            if !self.accepts_media_intent(request.target.generation) {
                continue;
            }
            // A command reply can precede its property-change event. Ask the
            // engine after the preceding edit has completed before planning
            // another edit, even when the previous edit left the chain equal.
            self.next_id += 1;
            let read = Operation::Read {
                name: request.target.chain.property().to_owned(),
                format: MpvPropertyFormat::Node,
            };
            match handle.request_property(self.next_id, &read) {
                Ok(()) => {
                    self.filter_edits.active = Some(InFlight::Read {
                        id: self.next_id,
                        deadline: Instant::now() + REPLY_TIMEOUT,
                        request,
                    });
                    return;
                }
                Err(error) => self.filter_edit_error(error.to_string()),
            }
        }
    }

    pub(super) fn take_filter_read(&mut self, event: &EventRef<'_>) -> Option<ReadResult> {
        if event.id() != event_id::GET_PROPERTY_REPLY {
            return None;
        }
        let InFlight::Read { request, .. } = self.filter_edits.active.take_if(
            |active| matches!(active, InFlight::Read { id, .. } if *id == event.userdata()),
        )?
        else {
            return None;
        };
        let filters = if let Some(error) = event.error() {
            Err(error)
        } else {
            event
                .client_reply()
                .map_err(|error| error.to_string())
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        };
        Some(ReadResult { request, filters })
    }

    pub(super) fn finish_filter_read(&mut self, handle: &Handle, result: ReadResult) {
        if self.accepts_media_intent(result.request.target.generation)
            && let Err(error) = self.apply_filter_edit(handle, result)
        {
            self.filter_edit_error(error);
        }
    }

    fn apply_filter_edit(&mut self, handle: &Handle, result: ReadResult) -> Result<(), String> {
        let filters = result.filters?;
        let Request { command, target } = result.request;
        if target
            .expected
            .as_ref()
            .is_some_and(|expected| expected != &filters)
        {
            return Err("The active filters changed. Select the filter again.".into());
        }
        let plan = command
            .plan(&target.chain.planning_state(filters))
            .map_err(|error| error.to_string())?;
        let [args] = plan.commands.as_slice() else {
            return Err("A filter edit must submit one native command".into());
        };
        let kind = match target.feedback {
            Some(name) => ReplyKind::for_filter_edit(args, name)
                .ok_or("A filter edit must add or remove a native filter")?,
            None => ReplyKind::Plain,
        };
        self.issue(handle, args, kind)?;
        self.filter_edits.active = Some(InFlight::Edit {
            id: self.next_id,
            deadline: Instant::now() + REPLY_TIMEOUT,
        });
        Ok(())
    }

    pub(super) fn retire_filter_edits(&mut self) {
        self.filter_edits.waiting.clear();
        if let Some(InFlight::Edit { id, .. }) = self.filter_edits.active.take() {
            self.pending.remove(&id);
        }
    }

    fn filter_edit_error(&mut self, error: String) {
        record_notice(&mut self.playback, NoticeSeverity::Error, error);
        self.mark_changed();
    }
}
