//! Portable policy for the Interface > Window media-size controls.
use serde::{Deserialize, Serialize};

use crate::window_aspect::PixelSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeOption {
    FitScreen,
    VideoSize05,
    VideoSize10,
    VideoSize15,
    VideoSize20,
}

/// Manual Video-menu sizing commands, separate from the automatic open policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualSizeCommand {
    Half,
    Normal,
    Double,
    FitScreen,
    Bigger,
    Smaller,
}

impl ManualSizeCommand {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Half),
            1 => Some(Self::Normal),
            2 => Some(Self::Double),
            3 => Some(Self::FitScreen),
            4 => Some(Self::Bigger),
            5 => Some(Self::Smaller),
            _ => None,
        }
    }
}

impl ResizeOption {
    pub const ALL: [Self; 5] = [
        Self::FitScreen,
        Self::VideoSize05,
        Self::VideoSize10,
        Self::VideoSize15,
        Self::VideoSize20,
    ];

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::FitScreen),
            1 => Some(Self::VideoSize05),
            2 => Some(Self::VideoSize10),
            3 => Some(Self::VideoSize15),
            4 => Some(Self::VideoSize20),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::FitScreen => 0,
            Self::VideoSize05 => 1,
            Self::VideoSize10 => 2,
            Self::VideoSize15 => 3,
            Self::VideoSize20 => 4,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::FitScreen => "Fit Screen",
            Self::VideoSize05 => "0.5x Video Size",
            Self::VideoSize10 => "1x Video Size",
            Self::VideoSize15 => "1.5x Video Size",
            Self::VideoSize20 => "2x Video Size",
        }
    }

    const fn factor(self) -> Option<f64> {
        match self {
            Self::FitScreen => None,
            Self::VideoSize05 => Some(0.5),
            Self::VideoSize10 => Some(1.0),
            Self::VideoSize15 => Some(1.5),
            Self::VideoSize20 => Some(2.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeTiming {
    Always,
    WhenOpenedManually,
    Never,
}

impl ResizeTiming {
    pub const ALL: [Self; 3] = [Self::Always, Self::WhenOpenedManually, Self::Never];

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Always),
            1 => Some(Self::WhenOpenedManually),
            2 => Some(Self::Never),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Always => 0,
            Self::WhenOpenedManually => 1,
            Self::Never => 2,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Always => "Always when playing a new file",
            Self::WhenOpenedManually => "When media is opened manually",
            Self::Never => "Do not resize",
        }
    }

    pub const fn applies_to(self, manually_opened: bool) -> bool {
        match self {
            Self::Always => true,
            Self::WhenOpenedManually => manually_opened,
            Self::Never => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub option: ResizeOption,
    pub timing: ResizeTiming,
    pub use_physical_resolution: bool,
    pub initial_geometry: InitialGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GeometryUnit {
    #[default]
    Points,
    ScreenPercent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SizeAxis {
    #[default]
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HorizontalAnchor {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAnchor {
    Bottom,
    #[default]
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InitialSize {
    pub axis: SizeAxis,
    pub value: u32,
    pub unit: GeometryUnit,
}

impl Default for InitialSize {
    fn default() -> Self {
        Self {
            axis: SizeAxis::Width,
            value: 1280,
            unit: GeometryUnit::Points,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InitialPosition {
    pub horizontal_anchor: HorizontalAnchor,
    pub horizontal_offset: u32,
    pub horizontal_unit: GeometryUnit,
    pub vertical_anchor: VerticalAnchor,
    pub vertical_offset: u32,
    pub vertical_unit: GeometryUnit,
}

impl Default for InitialPosition {
    fn default() -> Self {
        Self {
            horizontal_anchor: HorizontalAnchor::Left,
            horizontal_offset: 20,
            horizontal_unit: GeometryUnit::Points,
            vertical_anchor: VerticalAnchor::Top,
            vertical_offset: 20,
            vertical_unit: GeometryUnit::Points,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct InitialGeometry {
    pub size: Option<InitialSize>,
    pub position: Option<InitialPosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub size: PixelSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub size: PixelSize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            option: ResizeOption::VideoSize10,
            timing: ResizeTiming::WhenOpenedManually,
            use_physical_resolution: true,
            initial_geometry: InitialGeometry::default(),
        }
    }
}

/// Apply the optional initial geometry to a video-sized client area.
/// Percent sizes are relative to the selected screen's usable extent; point
/// values are converted by the platform window's current scale factor.
pub fn initial_placement(
    base_size: PixelSize,
    aspect: f64,
    display_scale: f64,
    screen: ScreenRect,
    minimum: PixelSize,
    geometry: InitialGeometry,
) -> Option<Placement> {
    if !base_size.valid()
        || !screen.size.valid()
        || !minimum.valid()
        || !crate::window_aspect::valid_aspect(aspect)
        || !display_scale.is_finite()
        || display_scale <= 0.0
    {
        return None;
    }

    let mut size = base_size;
    if let Some(initial) = geometry.size {
        if initial.value == 0
            || (initial.unit == GeometryUnit::ScreenPercent && initial.value > 100)
        {
            return None;
        }
        let screen_span = match initial.axis {
            SizeAxis::Width => screen.size.width,
            SizeAxis::Height => screen.size.height,
        };
        let requested = match initial.unit {
            GeometryUnit::Points => (f64::from(initial.value) * display_scale).round(),
            GeometryUnit::ScreenPercent => {
                f64::from(screen_span) * f64::from(initial.value) / 100.0
            }
        };
        let requested = pixels(requested);
        size = match initial.axis {
            SizeAxis::Width => PixelSize {
                width: requested,
                height: pixels(f64::from(requested) / aspect),
            },
            SizeAxis::Height => PixelSize {
                width: pixels(f64::from(requested) * aspect),
                height: requested,
            },
        };
    }

    size = size_at_aspect(
        f64::from(size.width).min(f64::from(screen.size.width)),
        aspect,
        Some(screen.size),
    );
    let min_scale = (f64::from(minimum.width) / f64::from(size.width))
        .max(f64::from(minimum.height) / f64::from(size.height));
    if min_scale > 1.0 {
        size = size_at_aspect(f64::from(size.width) * min_scale, aspect, Some(screen.size));
    }

    let (x, y) = if let Some(position) = geometry.position {
        if position.horizontal_offset == 0
            || position.vertical_offset == 0
            || (position.horizontal_unit == GeometryUnit::ScreenPercent
                && position.horizontal_offset > 100)
            || (position.vertical_unit == GeometryUnit::ScreenPercent
                && position.vertical_offset > 100)
        {
            return None;
        }
        let x_offset = geometry_offset(
            position.horizontal_offset,
            position.horizontal_unit,
            screen.size.width,
            display_scale,
        );
        let y_offset = geometry_offset(
            position.vertical_offset,
            position.vertical_unit,
            screen.size.height,
            display_scale,
        );
        let x = match (position.horizontal_anchor, position.horizontal_unit) {
            (HorizontalAnchor::Left, GeometryUnit::Points) => x_offset,
            (HorizontalAnchor::Right, GeometryUnit::Points) => screen
                .size
                .width
                .saturating_sub(x_offset)
                .saturating_sub(size.width),
            (HorizontalAnchor::Left, GeometryUnit::ScreenPercent) => {
                x_offset.saturating_sub(size.width / 2)
            }
            (HorizontalAnchor::Right, GeometryUnit::ScreenPercent) => screen
                .size
                .width
                .saturating_sub(x_offset)
                .saturating_sub(size.width / 2),
        };
        // IINA's geometry origin is bottom-left on macOS. Convert that intent
        // to each platform's top-left screen coordinates at this boundary.
        let y = match (position.vertical_anchor, position.vertical_unit) {
            (VerticalAnchor::Top, GeometryUnit::Points) => y_offset,
            (VerticalAnchor::Bottom, GeometryUnit::Points) => screen
                .size
                .height
                .saturating_sub(y_offset)
                .saturating_sub(size.height),
            (VerticalAnchor::Bottom, GeometryUnit::ScreenPercent) => screen
                .size
                .height
                .saturating_sub(y_offset)
                .saturating_sub(size.height / 2),
            (VerticalAnchor::Top, GeometryUnit::ScreenPercent) => {
                y_offset.saturating_sub(size.height / 2)
            }
        };
        (x, y)
    } else {
        (
            screen.size.width.saturating_sub(size.width) / 2,
            screen.size.height.saturating_sub(size.height) / 2,
        )
    };

    Some(Placement {
        x: screen
            .x
            .saturating_add(i32::try_from(x).unwrap_or(i32::MAX)),
        y: screen
            .y
            .saturating_add(i32::try_from(y).unwrap_or(i32::MAX)),
        size,
    })
}

fn geometry_offset(value: u32, unit: GeometryUnit, screen_span: u32, scale: f64) -> u32 {
    match unit {
        GeometryUnit::Points => pixels(f64::from(value) * scale),
        GeometryUnit::ScreenPercent => pixels(f64::from(screen_span) * f64::from(value) / 100.0),
    }
}

/// Derive a centered physical window size while retaining the displayed pixel ratio.
/// The caller supplies the monitor's usable bounds and the window's physical minimum.
pub fn target_size(
    video_width: f64,
    video_height: f64,
    rotation: Option<i64>,
    display_scale: f64,
    settings: Settings,
    maximum: Option<PixelSize>,
    minimum: PixelSize,
) -> Option<PixelSize> {
    if !video_width.is_finite()
        || !video_height.is_finite()
        || video_width <= 0.0
        || video_height <= 0.0
        || !display_scale.is_finite()
        || display_scale <= 0.0
    {
        return None;
    }
    let (width, height) = if rotation.is_some_and(|value| value.rem_euclid(180) == 90) {
        (video_height, video_width)
    } else {
        (video_width, video_height)
    };
    let scale = if settings.use_physical_resolution {
        1.0
    } else {
        display_scale
    };
    let width = width * scale;
    let height = height * scale;
    let aspect = width / height;
    let requested_scale = settings.option.factor().unwrap_or(1.0);
    let mut target = PixelSize {
        width: pixels(width * requested_scale),
        height: pixels(height * requested_scale),
    };

    let maximum = maximum.filter(|size| size.valid());
    if let Some(maximum) = maximum {
        let limit = (f64::from(maximum.width) / f64::from(target.width))
            .min(f64::from(maximum.height) / f64::from(target.height))
            .min(if settings.option == ResizeOption::FitScreen {
                1.0
            } else {
                f64::INFINITY
            });
        if limit < 1.0 {
            target = size_at_aspect(f64::from(target.width) * limit, aspect, Some(maximum));
        }
    } else {
        target = size_at_aspect(f64::from(target.width), aspect, None);
    }
    let min_width = f64::from(minimum.width) / f64::from(target.width);
    let min_height = f64::from(minimum.height) / f64::from(target.height);
    let grow = min_width.max(min_height);
    if grow > 1.0 {
        target = size_at_aspect(f64::from(target.width) * grow, aspect, maximum);
    }
    target.valid().then_some(target)
}

/// Current video and window measurements used by a manual resize command.
pub struct ManualSizeContext {
    pub current: PixelSize,
    pub video_width: f64,
    pub video_height: f64,
    pub rotation: Option<i64>,
    pub display_scale: f64,
    pub use_physical_resolution: bool,
    pub maximum: Option<PixelSize>,
    pub minimum: PixelSize,
}

/// Calculate a user-invoked Video-menu resize without changing the automatic
/// sizing preference. Sizes use physical pixels; `display_scale` converts
/// IINA's 25-point Bigger/Smaller increment on the current display.
pub fn manual_size_target(
    command: ManualSizeCommand,
    context: ManualSizeContext,
) -> Option<PixelSize> {
    let ManualSizeContext {
        current,
        video_width,
        video_height,
        rotation,
        display_scale,
        use_physical_resolution,
        maximum,
        minimum,
    } = context;
    if !current.valid() || !display_scale.is_finite() || display_scale <= 0.0 {
        return None;
    }

    if command == ManualSizeCommand::FitScreen {
        return fit_screen(current, maximum);
    }
    if !video_width.is_finite()
        || !video_height.is_finite()
        || video_width <= 0.0
        || video_height <= 0.0
    {
        return None;
    }

    let (width, height) = if rotation.is_some_and(|value| value.rem_euclid(180) == 90) {
        (video_height, video_width)
    } else {
        (video_width, video_height)
    };
    let scale = if use_physical_resolution {
        1.0
    } else {
        display_scale
    };
    let aspect = width / height;

    if matches!(
        command,
        ManualSizeCommand::Bigger | ManualSizeCommand::Smaller
    ) {
        let direction = if command == ManualSizeCommand::Bigger {
            1.0
        } else {
            -1.0
        };
        let requested_width =
            (f64::from(current.width) + direction * 25.0 * display_scale).max(1.0);
        return Some(satisfy_minimum(
            size_at_aspect(requested_width, aspect, None),
            aspect,
            minimum,
        ));
    }

    let command_scale = match command {
        ManualSizeCommand::Half => 0.5,
        ManualSizeCommand::Normal => 1.0,
        ManualSizeCommand::Double => 2.0,
        ManualSizeCommand::FitScreen | ManualSizeCommand::Bigger | ManualSizeCommand::Smaller => {
            return None;
        }
    };
    let requested = size_at_aspect(width * scale * command_scale, aspect, None);
    let target = if maximum
        .is_some_and(|bounds| requested.width > bounds.width || requested.height > bounds.height)
    {
        fit_screen(current, maximum)?
    } else {
        satisfy_minimum(requested, aspect, minimum)
    };
    target.valid().then_some(target)
}

fn fit_screen(current: PixelSize, maximum: Option<PixelSize>) -> Option<PixelSize> {
    let maximum = maximum.filter(|size| size.valid())?;
    let scale = (f64::from(maximum.width) / f64::from(current.width))
        .min(f64::from(maximum.height) / f64::from(current.height))
        .min(1.0);
    Some(PixelSize {
        width: pixels(f64::from(current.width) * scale),
        height: pixels(f64::from(current.height) * scale),
    })
}

fn satisfy_minimum(size: PixelSize, aspect: f64, minimum: PixelSize) -> PixelSize {
    if !minimum.valid() {
        return size;
    }
    let scale = (f64::from(minimum.width) / f64::from(size.width))
        .max(f64::from(minimum.height) / f64::from(size.height))
        .max(1.0);
    size_at_aspect(f64::from(size.width) * scale, aspect, None)
}

fn size_at_aspect(width: f64, aspect: f64, maximum: Option<PixelSize>) -> PixelSize {
    let mut width = width.max(1.0);
    let mut height = width / aspect;
    if let Some(maximum) = maximum.filter(|size| size.valid()) {
        let scale = (f64::from(maximum.width) / width)
            .min(f64::from(maximum.height) / height)
            .min(1.0);
        width *= scale;
        height *= scale;
    }
    PixelSize {
        width: pixels(width),
        height: pixels(height),
    }
}

fn pixels(value: f64) -> u32 {
    value.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(option: ResizeOption, timing: ResizeTiming, physical: bool) -> Settings {
        Settings {
            option,
            timing,
            use_physical_resolution: physical,
            initial_geometry: InitialGeometry::default(),
        }
    }

    #[test]
    fn defaults_and_interface_choices_match_iina() {
        let defaults = Settings::default();
        assert_eq!(defaults.option, ResizeOption::VideoSize10);
        assert_eq!(defaults.timing, ResizeTiming::WhenOpenedManually);
        assert!(defaults.use_physical_resolution);
        assert_eq!(
            ResizeOption::ALL.map(ResizeOption::label),
            [
                "Fit Screen",
                "0.5x Video Size",
                "1x Video Size",
                "1.5x Video Size",
                "2x Video Size"
            ]
        );
        assert_eq!(
            ResizeTiming::ALL.map(ResizeTiming::label),
            [
                "Always when playing a new file",
                "When media is opened manually",
                "Do not resize"
            ]
        );
    }

    #[test]
    fn video_size_modes_scale_physical_bounds_and_rotation() {
        let target = |option, scale, physical, rotation| {
            target_size(
                1920.0,
                1080.0,
                rotation,
                scale,
                settings(option, ResizeTiming::Always, physical),
                None,
                PixelSize {
                    width: 1,
                    height: 1,
                },
            )
            .unwrap()
        };
        assert_eq!(
            target(ResizeOption::VideoSize10, 1.0, true, None),
            PixelSize {
                width: 1920,
                height: 1080
            }
        );
        assert_eq!(
            target(ResizeOption::VideoSize05, 1.0, true, None),
            PixelSize {
                width: 960,
                height: 540
            }
        );
        assert_eq!(
            target(ResizeOption::VideoSize15, 1.0, true, None),
            PixelSize {
                width: 2880,
                height: 1620
            }
        );
        assert_eq!(
            target(ResizeOption::VideoSize20, 1.0, true, None),
            PixelSize {
                width: 3840,
                height: 2160
            }
        );
        assert_eq!(
            target(ResizeOption::VideoSize10, 2.0, false, None),
            PixelSize {
                width: 3840,
                height: 2160
            }
        );
        assert_eq!(
            target(ResizeOption::VideoSize10, 1.0, true, Some(90)),
            PixelSize {
                width: 1080,
                height: 1920
            }
        );
    }

    #[test]
    fn fit_screen_only_shrinks_and_preserves_ratio_and_minimum_size() {
        let settings = settings(ResizeOption::FitScreen, ResizeTiming::Always, true);
        assert_eq!(
            target_size(
                1920.0,
                1080.0,
                None,
                1.0,
                settings,
                Some(PixelSize {
                    width: 1280,
                    height: 720
                }),
                PixelSize {
                    width: 520,
                    height: 300
                },
            ),
            Some(PixelSize {
                width: 1280,
                height: 720
            })
        );
        assert_eq!(
            target_size(
                320.0,
                180.0,
                None,
                1.0,
                settings,
                Some(PixelSize {
                    width: 1280,
                    height: 720
                }),
                PixelSize {
                    width: 520,
                    height: 300
                },
            ),
            Some(PixelSize {
                width: 533,
                height: 300
            })
        );
    }

    #[test]
    fn resize_timing_preserves_manual_open_semantics() {
        assert!(ResizeTiming::Always.applies_to(false));
        assert!(ResizeTiming::WhenOpenedManually.applies_to(true));
        assert!(!ResizeTiming::WhenOpenedManually.applies_to(false));
        assert!(!ResizeTiming::Never.applies_to(true));
    }

    #[test]
    fn initial_geometry_scales_points_and_centers_a_size_only_override() {
        let placement = initial_placement(
            PixelSize {
                width: 960,
                height: 540,
            },
            16.0 / 9.0,
            2.0,
            ScreenRect {
                x: 100,
                y: 50,
                size: PixelSize {
                    width: 2000,
                    height: 1200,
                },
            },
            PixelSize {
                width: 520,
                height: 300,
            },
            InitialGeometry {
                size: Some(InitialSize {
                    axis: SizeAxis::Width,
                    value: 800,
                    unit: GeometryUnit::Points,
                }),
                position: None,
            },
        )
        .unwrap();
        assert_eq!(
            placement.size,
            PixelSize {
                width: 1600,
                height: 900
            }
        );
        assert_eq!((placement.x, placement.y), (300, 200));
    }

    #[test]
    fn initial_geometry_positions_from_screen_edges_and_percent_centers() {
        let base = PixelSize {
            width: 800,
            height: 450,
        };
        let screen = ScreenRect {
            x: -1920,
            y: 40,
            size: PixelSize {
                width: 1920,
                height: 1080,
            },
        };
        let point_position = initial_placement(
            base,
            16.0 / 9.0,
            1.0,
            screen,
            PixelSize {
                width: 520,
                height: 300,
            },
            InitialGeometry {
                size: None,
                position: Some(InitialPosition {
                    horizontal_anchor: HorizontalAnchor::Right,
                    horizontal_offset: 24,
                    horizontal_unit: GeometryUnit::Points,
                    vertical_anchor: VerticalAnchor::Bottom,
                    vertical_offset: 30,
                    vertical_unit: GeometryUnit::Points,
                }),
            },
        )
        .unwrap();
        // The right anchor is 24 points inward from the physical screen edge.
        assert_eq!((point_position.x, point_position.y), (-824, 640));

        let percent_position = initial_placement(
            base,
            16.0 / 9.0,
            1.0,
            screen,
            PixelSize {
                width: 520,
                height: 300,
            },
            InitialGeometry {
                size: None,
                position: Some(InitialPosition {
                    horizontal_anchor: HorizontalAnchor::Left,
                    horizontal_offset: 50,
                    horizontal_unit: GeometryUnit::ScreenPercent,
                    vertical_anchor: VerticalAnchor::Top,
                    vertical_offset: 50,
                    vertical_unit: GeometryUnit::ScreenPercent,
                }),
            },
        )
        .unwrap();
        assert_eq!((percent_position.x, percent_position.y), (-1360, 355));
    }

    #[test]
    fn initial_geometry_rejects_invalid_percentages_and_keeps_aspect_within_screen() {
        let screen = ScreenRect {
            x: 0,
            y: 0,
            size: PixelSize {
                width: 1280,
                height: 720,
            },
        };
        let minimum = PixelSize {
            width: 520,
            height: 300,
        };
        let oversized = initial_placement(
            PixelSize {
                width: 640,
                height: 360,
            },
            16.0 / 9.0,
            1.0,
            screen,
            minimum,
            InitialGeometry {
                size: Some(InitialSize {
                    axis: SizeAxis::Height,
                    value: 100,
                    unit: GeometryUnit::ScreenPercent,
                }),
                position: None,
            },
        )
        .unwrap();
        assert_eq!(
            oversized.size,
            PixelSize {
                width: 1280,
                height: 720
            }
        );

        let invalid = InitialGeometry {
            size: Some(InitialSize {
                axis: SizeAxis::Width,
                value: 101,
                unit: GeometryUnit::ScreenPercent,
            }),
            position: None,
        };
        assert!(
            initial_placement(
                PixelSize {
                    width: 640,
                    height: 360,
                },
                16.0 / 9.0,
                1.0,
                screen,
                minimum,
                invalid,
            )
            .is_none()
        );
    }

    #[test]
    fn choices_round_trip_and_reject_out_of_range_indexes() {
        for (index, choice) in ResizeOption::ALL.into_iter().enumerate() {
            assert_eq!(ResizeOption::from_index(index as i32), Some(choice));
            assert_eq!(choice.index(), index as i32);
        }
        for (index, choice) in ResizeTiming::ALL.into_iter().enumerate() {
            assert_eq!(ResizeTiming::from_index(index as i32), Some(choice));
            assert_eq!(choice.index(), index as i32);
        }
        assert_eq!(ResizeOption::from_index(-1), None);
        assert_eq!(ResizeTiming::from_index(3), None);
    }
}
