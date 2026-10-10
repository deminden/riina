//! Ownership of one interactive crop edit, tied to its source generation.
use crate::{Command, Phase, Playback, crop::Rectangle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    generation: u64,
    width: u32,
    height: u32,
    was_paused: bool,
}

impl Session {
    pub fn begin(state: &Playback) -> Result<Self, String> {
        let video = state
            .video
            .as_ref()
            .ok_or("Source video dimensions are unavailable")?;
        if video.w == 0
            || video.h == 0
            || state.video_filters.is_none()
            || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
        {
            return Err("The source video is not ready for cropping".into());
        }
        Ok(Self {
            generation: state.generation,
            width: video.w,
            height: video.h,
            was_paused: state.paused,
        })
    }

    pub fn whole_source(&self) -> Rectangle {
        Rectangle {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        }
    }

    pub fn validate(&self, state: &Playback) -> Result<(), String> {
        if state.generation != self.generation
            || state.video.as_ref().map(|video| (video.w, video.h))
                != Some((self.width, self.height))
            || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
        {
            return Err("The video changed while cropping".into());
        }
        Ok(())
    }

    pub fn ready(&self, state: &Playback) -> bool {
        self.validate(state).is_ok()
            && state.paused
            && state.video_filters.as_ref().is_some_and(|filters| {
                !filters
                    .iter()
                    .any(|filter| filter.label.as_deref() == Some(crate::crop::FILTER_LABEL))
            })
    }

    pub fn enter_commands(&self, state: &Playback) -> Result<Vec<Vec<String>>, String> {
        self.validate(state)?;
        let mut commands = Command::VideoCrop(None).plan(state)?.commands;
        commands.push(vec!["set".into(), "pause".into(), "yes".into()]);
        Ok(commands)
    }

    pub fn exit_commands(
        &self,
        state: &Playback,
        rectangle: Option<Rectangle>,
    ) -> Result<Vec<Vec<String>>, String> {
        self.validate(state)?;
        let mut commands = match rectangle {
            Some(rectangle) => Command::VideoCropRectangle(rectangle).plan(state)?.commands,
            // Entry already cleared the old crop. Cancel must not reapply it.
            None => Vec::new(),
        };
        commands.push(vec![
            "set".into(),
            "pause".into(),
            if self.was_paused { "yes" } else { "no" }.into(),
        ]);
        Ok(commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(paused: bool) -> Playback {
        Playback {
            generation: 7,
            phase: if paused {
                Phase::Paused
            } else {
                Phase::Playing
            },
            paused,
            video: Some(serde_json::from_value(serde_json::json!({"w":320,"h":180})).unwrap()),
            video_filters: Some(vec![]),
            ..Default::default()
        }
    }

    #[test]
    fn cancellation_restores_prior_pause_without_reapplying_crop() {
        for was_paused in [false, true] {
            let mut state = source(was_paused);
            let session = Session::begin(&state).unwrap();
            assert_eq!(
                session.whole_source(),
                Rectangle {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 180
                }
            );
            assert_eq!(
                session.enter_commands(&state).unwrap(),
                vec![vec!["set", "pause", "yes"]]
            );
            state.paused = true;
            assert!(session.ready(&state));
            assert_eq!(
                session.exit_commands(&state, None).unwrap(),
                vec![vec!["set", "pause", if was_paused { "yes" } else { "no" }]]
            );
            state.generation += 1;
            assert!(!session.ready(&state));
            assert!(session.exit_commands(&state, None).is_err());
        }
    }

    #[test]
    fn stale_source_or_invalid_confirmation_produces_no_partial_plan() {
        let mut state = source(false);
        let session = Session::begin(&state).unwrap();
        let invalid = Rectangle {
            width: 321,
            ..session.whole_source()
        };
        assert!(session.exit_commands(&state, Some(invalid)).is_err());
        state.video.as_mut().unwrap().w = 640;
        assert!(session.enter_commands(&state).is_err());
        assert!(
            session
                .exit_commands(&state, Some(session.whole_source()))
                .is_err()
        );
        state.video = None;
        assert!(Session::begin(&state).is_err());
    }
}
