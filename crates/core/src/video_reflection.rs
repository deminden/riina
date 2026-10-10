//! Independent managed axes; reflecting a picture must not replace its crop.
use crate::{Playback, crop::VideoFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub fn filter(self) -> String {
        format!(
            "@{}:{}",
            self.label(),
            match self {
                Self::Horizontal => "hflip",
                Self::Vertical => "vflip",
            }
        )
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "riina-mirror",
            Self::Vertical => "riina-flip",
        }
    }

    pub fn observed(self, filters: Option<&[VideoFilter]>) -> Option<bool> {
        filters.map(|filters| {
            filters
                .iter()
                .any(|filter| filter.label.as_deref() == Some(self.label()))
        })
    }

    pub fn commands(self, enabled: bool, state: &Playback) -> Result<Vec<Vec<String>>, String> {
        let current = self
            .observed(state.video_filters.as_deref())
            .ok_or("Video filters are unavailable")?;
        if current == enabled {
            return Ok(Vec::new());
        }
        Ok(vec![if enabled {
            vec!["vf".into(), "add".into(), self.filter()]
        } else {
            vec!["vf".into(), "remove".into(), format!("@{}", self.label())]
        }])
    }
}
