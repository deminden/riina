use super::{Command, cmd, flag, set};
use crate::{Playback, crop, delogo};

impl Command {
    pub(super) fn video_args(&self) -> Result<Vec<String>, String> {
        let args = match self {
            Self::VideoAspect(aspect) => set("video-aspect-override", aspect.native_value()),
            // Read the native chain at execution; rapid activations must not
            // invert the same stale observed state twice.
            Self::ToggleVideoReflection(axis) => vec!["vf".into(), "toggle".into(), axis.filter()],
            Self::VideoRotation(rotation) => set("video-rotate", rotation.degrees().to_string()),
            Self::Deinterlace(enabled) => set("deinterlace", flag(*enabled)),
            // Limit the native cycle to the reference's two states; newer mpv
            // also offers auto, which is not a third state in IINA's checkbox.
            Self::ToggleDeinterlace => cmd(["cycle-values", "deinterlace", "no", "yes"]),
            Self::HardwareDecoding(mode) => set("hwdec", mode.native_value()),
            Self::VideoEqualizer { option, value } if (-100..=100).contains(value) => {
                set(option.property(), value.to_string())
            }
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }
}

pub(super) fn remove_delogo(state: &Playback) -> Result<Vec<Vec<String>>, String> {
    let filters = state
        .video_filters
        .as_ref()
        .ok_or("Video filters are unavailable")?;
    Ok(
        if filters
            .iter()
            .any(|filter| filter.label.as_deref() == Some(delogo::FILTER_LABEL))
        {
            delogo::remove_filter()
        } else {
            Vec::new()
        },
    )
}

pub(super) fn crop_rectangle(
    state: &Playback,
    rectangle: &crop::Rectangle,
) -> Result<Vec<Vec<String>>, String> {
    let source = state
        .video
        .as_ref()
        .ok_or("Source video dimensions are unavailable")?;
    if rectangle.x == 0
        && rectangle.y == 0
        && rectangle.width != 0
        && rectangle.height != 0
        && rectangle.width == source.w
        && rectangle.height == source.h
    {
        return crop_preset(state, None);
    }
    Ok(vec![vec![
        "vf".into(),
        "add".into(),
        rectangle.editor_filter(source.w, source.h)?,
    ]])
}

pub(super) fn crop_preset(
    state: &Playback,
    preset: Option<crop::Preset>,
) -> Result<Vec<Vec<String>>, String> {
    let commands = match preset {
        Some(preset) => {
            let source = state
                .video
                .as_ref()
                .ok_or("Source video dimensions are unavailable")?;
            vec![vec![
                "vf".into(),
                "add".into(),
                preset.filter(source.w, source.h)?,
            ]]
        }
        None => {
            let filters = state
                .video_filters
                .as_ref()
                .ok_or("Video filters are unavailable")?;
            if filters
                .iter()
                .any(|filter| filter.label.as_deref() == Some(crop::FILTER_LABEL))
            {
                vec![vec![
                    "vf".into(),
                    "remove".into(),
                    format!("@{}", crop::FILTER_LABEL),
                ]]
            } else {
                Vec::new()
            }
        }
    };
    Ok(commands)
}

pub(super) fn hdr_primaries(state: &Playback, enabled: bool) -> Result<Option<&str>, String> {
    let primaries = if enabled {
        let source = state
            .video
            .as_ref()
            .or(state.video_target.as_ref())
            .or(state.video_output.as_ref())
            .filter(|video| video.is_hdr())
            .ok_or("HDR output is available only for a recognized HDR source")?;
        source.primaries.as_deref()
    } else {
        None
    };
    Ok(primaries)
}
