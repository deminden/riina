use std::{ffi::NulError, fmt, io, path::PathBuf, sync::mpsc};

/// Playback start, native calls and command admission can fail independently.
#[derive(Debug)]
pub enum EngineError {
    LibraryMissing(PathBuf),
    UnsupportedApi(u64),
    LibraryPath(io::Error),
    LibraryLoad(libloading::Error),
    LibraryRegistryUnavailable,
    Native {
        call: &'static str,
        message: String,
    },
    QueueFull,
    Stopped,
    PropertyTimeout,
    ReservedShutdown,
    #[cfg(feature = "test-support")]
    ObserverAlreadyInstalled,
    InvalidArgument {
        reason: &'static str,
    },
    InvalidFrameReply {
        reason: &'static str,
    },
    Frame(riina_core::FrameError),
    WorkerStart(io::Error),
    WorkerHandshake(mpsc::RecvError),
    StartupOption(NulError),
    RequiredStartupOption {
        name: &'static str,
        source: Box<EngineError>,
    },
    WindowsApartment(i32),
    Render {
        call: &'static str,
        message: String,
    },
    ThumbnailWorker(io::Error),
    ThumbnailCancelled,
    ThumbnailNoDuration,
    ThumbnailNoVideo,
    ThumbnailNoPixels,
    ThumbnailStopped,
    Thumbnail {
        message: String,
    },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LibraryMissing(path) => {
                let remedy = if cfg!(windows) {
                    "Run scripts/fetch-mpv.ps1."
                } else {
                    "Install libmpv or place the matching library beside Riina."
                };
                write!(f, "libmpv is missing at {}. {remedy}", path.display())
            }
            Self::UnsupportedApi(version) => write!(
                f,
                "Unsupported libmpv client API {version:#x}; require 2.5 or a compatible newer 2.x"
            ),
            Self::LibraryPath(error) | Self::WorkerStart(error) => write!(f, "{error}"),
            Self::LibraryLoad(error) => write!(f, "{error}"),
            Self::LibraryRegistryUnavailable => {
                f.write_str("The native engine library registry is unavailable")
            }
            Self::Native { message, .. }
            | Self::Render { message, .. }
            | Self::Thumbnail { message } => f.write_str(message),
            Self::QueueFull => {
                f.write_str("Player command was not accepted: sending on a full channel")
            }
            Self::Stopped => f.write_str("The playback engine has stopped"),
            Self::PropertyTimeout => f.write_str("The native property request timed out"),
            Self::ReservedShutdown => {
                f.write_str("Player shutdown is reserved for its final owner")
            }
            #[cfg(feature = "test-support")]
            Self::ObserverAlreadyInstalled => {
                f.write_str("A recording engine already has a playback observer")
            }
            Self::InvalidArgument { reason } | Self::InvalidFrameReply { reason } => {
                f.write_str(reason)
            }
            Self::Frame(error) => error.fmt(f),
            Self::WorkerHandshake(error) => write!(f, "{error}"),
            Self::StartupOption(error) => write!(f, "{error}"),
            Self::RequiredStartupOption { name, source } => {
                write!(f, "Could not set required libmpv option {name}: {source}")
            }
            Self::WindowsApartment(error) => {
                write!(f, "Could not initialize Windows playback COM: {error:#x}")
            }
            Self::ThumbnailCancelled => f.write_str("Thumbnail extraction was cancelled"),
            Self::ThumbnailNoDuration => {
                f.write_str("The media duration is unavailable for thumbnail extraction")
            }
            Self::ThumbnailNoVideo => f.write_str("The media has no video frames to preview"),
            Self::ThumbnailNoPixels => {
                f.write_str("libmpv completed a frame capture without returning pixels")
            }
            Self::ThumbnailStopped => f.write_str("The thumbnail decoder stopped unexpectedly"),
            Self::ThumbnailWorker(error) => {
                write!(f, "Could not start thumbnail extraction: {error}")
            }
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LibraryPath(error) | Self::WorkerStart(error) | Self::ThumbnailWorker(error) => {
                Some(error)
            }
            Self::LibraryLoad(error) => Some(error),
            Self::WorkerHandshake(error) => Some(error),
            Self::StartupOption(error) => Some(error),
            Self::RequiredStartupOption { source, .. } => Some(source.as_ref()),
            Self::Frame(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EngineError> for String {
    fn from(error: EngineError) -> Self {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::EngineError;
    use std::error::Error;

    #[test]
    fn required_startup_option_reports_its_name_and_native_cause() {
        let error = EngineError::RequiredStartupOption {
            name: "load-scripts",
            source: Box::new(EngineError::Native {
                call: "mpv_set_option_string",
                message: "option not found".into(),
            }),
        };

        assert_eq!(
            error.to_string(),
            "Could not set required libmpv option load-scripts: option not found"
        );
        assert!(matches!(
            error.source().unwrap().downcast_ref::<EngineError>(),
            Some(EngineError::Native {
                call: "mpv_set_option_string",
                ..
            })
        ));
    }
}
