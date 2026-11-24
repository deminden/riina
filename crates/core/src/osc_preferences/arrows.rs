//! Actions and speed press state for the on-screen transport arrows.
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrowAction {
    #[default]
    Speed,
    Playlist,
    Seek,
}

impl ArrowAction {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    #[must_use]
    pub fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Speed),
            1 => Some(Self::Playlist),
            2 => Some(Self::Seek),
            _ => None,
        }
    }

    #[must_use]
    pub const fn index(self) -> i32 {
        match self {
            Self::Speed => 0,
            Self::Playlist => 1,
            Self::Seek => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrowDirection {
    Left,
    Right,
}

impl ArrowDirection {
    const fn sign(self) -> i8 {
        match self {
            Self::Left => -1,
            Self::Right => 1,
        }
    }

    #[must_use]
    pub fn seek_seconds(self) -> f32 {
        f32::from(self.sign()) * 10.0
    }
}

const EXTREME_STEP: i8 = 5;
const LONG_PRESS: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
pub struct ArrowSpeed {
    step: i8,
    press: Option<Press>,
}

#[derive(Debug)]
struct Press {
    direction: ArrowDirection,
    starting_step: i8,
    stage: i8,
    strongest_stage: i8,
}

impl ArrowSpeed {
    #[must_use]
    pub fn speed(&self) -> f64 {
        2.0_f64.powi(i32::from(self.step))
    }

    /// Reversing the arrow direction begins from normal playback speed.
    pub fn press(&mut self, direction: ArrowDirection) -> Option<f64> {
        if self.press.is_some() {
            return None;
        }
        let starting_step = if self.step.signum() == direction.sign() {
            self.step
        } else {
            0
        };
        let next = starting_step + direction.sign();
        self.step = if next.abs() > EXTREME_STEP { 0 } else { next };
        self.press = Some(Press {
            direction,
            starting_step,
            stage: 1,
            strongest_stage: 1,
        });
        Some(self.speed())
    }

    pub fn pressure(&mut self, stage: i64) -> Option<f64> {
        let press = self.press.as_mut()?;
        let stage = i8::try_from(stage.min(i64::from(EXTREME_STEP))).ok()?;
        if stage < 1 || stage == press.stage {
            return None;
        }
        press.stage = stage;
        press.strongest_stage = press.strongest_stage.max(stage);
        self.step = (press.starting_step + press.direction.sign() * stage)
            .clamp(-EXTREME_STEP, EXTREME_STEP);
        Some(self.speed())
    }

    /// A long first press or additional pressure temporarily changes playback speed.
    /// Repeated clicks can select and keep a stronger rate.
    pub fn release(&mut self, direction: ArrowDirection, elapsed: Duration) -> Option<f64> {
        if self.press.as_ref()?.direction != direction {
            return None;
        }
        let press = self.press.take()?;
        if press.strongest_stage > 1 || (elapsed >= LONG_PRESS && self.step.abs() <= 1) {
            self.step = 0;
        }
        Some(self.speed())
    }

    pub fn cancel(&mut self) -> Option<f64> {
        self.press.take()?;
        self.step = 0;
        Some(self.speed())
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArrowDirection::{Left, Right},
        *,
    };

    #[test]
    fn short_presses_step_from_normal_when_direction_reverses() {
        let mut speed = ArrowSpeed::default();
        for (direction, expected) in [(Right, 2.0), (Right, 4.0), (Left, 0.5), (Left, 0.25)] {
            assert_eq!(speed.press(direction), Some(expected));
            assert_eq!(speed.release(direction, Duration::ZERO), Some(expected));
        }
        assert_eq!(speed.cancel(), None);
        assert_eq!(speed.release(Left, Duration::ZERO), None);
    }

    #[test]
    fn long_presses_and_pressure_restore_normal_speed_on_release() {
        let mut speed = ArrowSpeed::default();
        assert_eq!(speed.press(Right), Some(2.0));
        assert_eq!(speed.release(Right, LONG_PRESS), Some(1.0));
        assert_eq!(speed.press(Left), Some(0.5));
        assert_eq!(speed.press(Right), None);
        assert_eq!(speed.pressure(3), Some(0.125));
        assert_eq!(speed.pressure(2), Some(0.25));
        assert_eq!(speed.pressure(1), Some(0.5));
        assert_eq!(speed.pressure(0), None);
        assert_eq!(speed.release(Right, Duration::ZERO), None);
        assert_eq!(speed.release(Left, Duration::ZERO), Some(1.0));
        assert_eq!(speed.press(Left), Some(0.5));
        assert_eq!(speed.cancel(), Some(1.0));
        assert_eq!(speed.release(Left, Duration::ZERO), None);
    }
}
