use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropError {
    EmptyRectangle,
    RectangleOutsideVideo,
    SourceTooSmall,
    RectangleOutsideSource,
    DisplayDimensionsUnavailable,
    DisplayRotationUnavailable,
    UnsupportedDisplayRotation,
    InvalidProjectionDimensions,
    ProjectionOutOfRange,
    EmptySelection,
    InvalidPointer,
    InvalidDrag,
    DelogoOutsideVideo,
}

impl fmt::Display for CropError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyRectangle => "The crop rectangle has no area",
            Self::RectangleOutsideVideo => "The crop rectangle is outside the source video",
            Self::SourceTooSmall => "The source is too small for this crop",
            Self::RectangleOutsideSource => "Crop rectangle is outside the source",
            Self::DisplayDimensionsUnavailable => "Video display dimensions are unavailable",
            Self::DisplayRotationUnavailable => "Video display rotation is unavailable",
            Self::UnsupportedDisplayRotation => {
                "This display rotation is not supported by the crop projection"
            }
            Self::InvalidProjectionDimensions => "Crop projection dimensions are invalid",
            Self::ProjectionOutOfRange => "Crop projection exceeds the numeric range",
            Self::EmptySelection => "Crop selection has no area",
            Self::InvalidPointer => "Crop pointer position is invalid",
            Self::InvalidDrag => "Crop drag geometry is invalid",
            Self::DelogoOutsideVideo => "The delogo selection is outside the source video",
        })
    }
}

impl std::error::Error for CropError {}
