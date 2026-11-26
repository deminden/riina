use super::{Command, CommandError, cmd};
use crate::{Playback, crop::VideoFilter, video_filters};

pub(super) fn plan_edit(
    command: &Command,
    state: &Playback,
) -> Option<Result<Vec<Vec<String>>, CommandError>> {
    Some(match command {
        Command::ToggleVideoFilter(filter) => FilterChain::Video.toggle(state, filter),
        Command::AddVideoFilter(filter) => FilterChain::Video.add(filter),
        Command::RemoveVideoFilter(filter) => FilterChain::Video.remove(state, filter),
        Command::RemoveVideoFilterAt(index) => FilterChain::Video.remove_index(state, *index),
        Command::SetVideoFilterAt { index, filter } => {
            FilterChain::Video.replace(state, *index, filter)
        }
        Command::ToggleAudioFilter(filter) => FilterChain::Audio.toggle(state, filter),
        Command::AddAudioFilter(filter) => FilterChain::Audio.add(filter),
        Command::RemoveAudioFilter(filter) => FilterChain::Audio.remove(state, filter),
        Command::RemoveAudioFilterAt(index) => FilterChain::Audio.remove_index(state, *index),
        Command::SetAudioFilterAt { index, filter } => {
            FilterChain::Audio.replace(state, *index, filter)
        }
        _ => return None,
    })
}

#[derive(Clone, Copy)]
enum FilterChain {
    Video,
    Audio,
}

impl FilterChain {
    const fn property(self) -> &'static str {
        match self {
            Self::Video => "vf",
            Self::Audio => "af",
        }
    }

    fn filters(self, state: &Playback) -> Result<&[VideoFilter], CommandError> {
        match self {
            Self::Video => state
                .video_filters
                .as_ref()
                .ok_or(CommandError::Unavailable {
                    what: "Video filters are unavailable",
                }),
            Self::Audio => state
                .audio_filters
                .as_ref()
                .ok_or(CommandError::Unavailable {
                    what: "Audio filters are unavailable",
                }),
        }
        .map(Vec::as_slice)
    }

    const fn invalid(self) -> &'static str {
        match self {
            Self::Video => "The video filter string is invalid",
            Self::Audio => "The audio filter string is invalid",
        }
    }

    const fn inactive(self) -> &'static str {
        match self {
            Self::Video => "The selected video filter is no longer active",
            Self::Audio => "The selected audio filter is no longer active",
        }
    }

    fn toggle(self, state: &Playback, filter: &str) -> Result<Vec<Vec<String>>, CommandError> {
        if !video_filters::valid_filter_string(filter) {
            return Err(CommandError::InvalidArgument {
                what: match self {
                    Self::Video => "The saved video filter string is invalid",
                    Self::Audio => "The saved audio filter string is invalid",
                },
            });
        }
        let filters = self.filters(state)?;
        let selected = filters.iter().position(|candidate| {
            video_filters::is_active(filter, std::slice::from_ref(candidate))
        });
        if let Some(index) = selected {
            Ok(self.remove_at(filters, index))
        } else {
            self.add(filter)
        }
    }

    fn add(self, filter: &str) -> Result<Vec<Vec<String>>, CommandError> {
        if !video_filters::valid_filter_string(filter) {
            return Err(CommandError::InvalidArgument {
                what: self.invalid(),
            });
        }
        Ok(vec![cmd([self.property(), "add", filter])])
    }

    fn remove(
        self,
        state: &Playback,
        selected: &VideoFilter,
    ) -> Result<Vec<Vec<String>>, CommandError> {
        let filters = self.filters(state)?;
        let index = filters
            .iter()
            .position(|candidate| candidate == selected)
            .ok_or(CommandError::Unavailable {
                what: self.inactive(),
            })?;
        Ok(self.remove_at(filters, index))
    }

    fn remove_index(
        self,
        state: &Playback,
        index: usize,
    ) -> Result<Vec<Vec<String>>, CommandError> {
        let filters = self.filters(state)?;
        if index >= filters.len() {
            return Err(CommandError::Unavailable {
                what: self.inactive(),
            });
        }
        Ok(self.remove_at(filters, index))
    }

    fn remove_at(self, filters: &[VideoFilter], index: usize) -> Vec<Vec<String>> {
        // The native object-list parser has no numeric delete. Replacing the
        // chain without the selected slot leaves the other entries in order.
        let chain = filters
            .iter()
            .enumerate()
            .filter(|(candidate, _)| *candidate != index)
            .map(|(_, filter)| video_filters::chain_argument(filter))
            .collect::<Vec<_>>()
            .join(",");
        vec![cmd([self.property(), "set", &chain])]
    }

    fn replace(
        self,
        state: &Playback,
        index: usize,
        filter: &str,
    ) -> Result<Vec<Vec<String>>, CommandError> {
        if !video_filters::valid_filter_string(filter) {
            return Err(CommandError::InvalidArgument {
                what: self.invalid(),
            });
        }
        let filters = self.filters(state)?;
        if index >= filters.len() {
            return Err(CommandError::Unavailable {
                what: self.inactive(),
            });
        }
        let mut chain: Vec<_> = filters.iter().map(video_filters::chain_argument).collect();
        chain[index] = filter.to_owned();
        Ok(vec![cmd([self.property(), "set", &chain.join(",")])])
    }
}
