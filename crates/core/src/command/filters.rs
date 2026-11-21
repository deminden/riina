use super::cmd;
use crate::{Playback, crop::VideoFilter, video_filters};

#[derive(Clone, Copy)]
pub(super) enum FilterChain {
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

    fn filters(self, state: &Playback) -> Result<&[VideoFilter], &'static str> {
        match self {
            Self::Video => state
                .video_filters
                .as_ref()
                .ok_or("Video filters are unavailable"),
            Self::Audio => state
                .audio_filters
                .as_ref()
                .ok_or("Audio filters are unavailable"),
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

    pub(super) fn toggle(self, state: &Playback, filter: &str) -> Result<Vec<Vec<String>>, String> {
        if !video_filters::valid_filter_string(filter) {
            return Err(match self {
                Self::Video => "The saved video filter string is invalid",
                Self::Audio => "The saved audio filter string is invalid",
            }
            .into());
        }
        let filters = self.filters(state)?;
        let selected = filters.iter().position(|candidate| {
            video_filters::is_active(filter, std::slice::from_ref(candidate))
        });
        let action = if let Some(index) = selected {
            cmd([self.property(), "del", &index.to_string()])
        } else {
            cmd([self.property(), "add", filter])
        };
        Ok(vec![action])
    }

    pub(super) fn add(self, filter: &str) -> Result<Vec<Vec<String>>, String> {
        if !video_filters::valid_filter_string(filter) {
            return Err(self.invalid().into());
        }
        Ok(vec![cmd([self.property(), "add", filter])])
    }

    pub(super) fn remove(
        self,
        state: &Playback,
        selected: &VideoFilter,
    ) -> Result<Vec<Vec<String>>, String> {
        let index = self
            .filters(state)?
            .iter()
            .position(|candidate| candidate == selected)
            .ok_or(self.inactive())?;
        Ok(vec![cmd([self.property(), "del", &index.to_string()])])
    }

    pub(super) fn replace(
        self,
        state: &Playback,
        index: usize,
        filter: &str,
    ) -> Result<Vec<Vec<String>>, String> {
        if !video_filters::valid_filter_string(filter) {
            return Err(self.invalid().into());
        }
        let filters = self.filters(state)?;
        if index >= filters.len() {
            return Err(self.inactive().into());
        }
        let mut chain: Vec<_> = filters.iter().map(video_filters::format_filter).collect();
        chain[index] = filter.to_owned();
        Ok(vec![cmd([self.property(), "set", &chain.join(",")])])
    }
}
