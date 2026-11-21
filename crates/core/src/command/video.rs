use super::{Command, cmd, flag, set};

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
