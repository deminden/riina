//! Fullscreen clock and battery details follow the player's visible control state.
use crate::PlayerWindow;
use battery::units::ratio::percent;
use chrono::Local;
use slint::{ComponentHandle, Timer, TimerMode};
use std::{rc::Rc, time::Duration};

const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

fn current_battery_percent() -> Option<f32> {
    let manager = battery::Manager::new().ok()?;
    manager
        .batteries()
        .ok()?
        .filter_map(Result::ok)
        .map(|battery| battery.state_of_charge().get::<percent>())
        .find(|value| value.is_finite() && (0.0..=100.0).contains(value))
}

fn battery_label(value: f32) -> Option<String> {
    (value.is_finite() && (0.0..=100.0).contains(&value))
        .then(|| format!("{}%", value.round() as u8))
}

fn update(app: &PlayerWindow) {
    // Use the local clock and minute precision; the system's locale-specific
    // 12/24-hour display preference remains a platform adaptation to refine.
    app.set_fullscreen_info_time(Local::now().format("%H:%M").to_string().into());
    let battery = current_battery_percent();
    app.set_fullscreen_info_battery(battery.and_then(battery_label).unwrap_or_default().into());
    app.set_fullscreen_info_battery_level(battery.unwrap_or_default());
}

fn should_show(app: &PlayerWindow) -> bool {
    app.get_fullscreen()
        && app.get_display_time_and_battery_in_fullscreen()
        && app.get_controls_visible()
        && app.get_loaded()
        && !app.get_crop_editing()
}

pub(crate) fn connect(app: &PlayerWindow) {
    let timer = Rc::new(Timer::default());
    let timer_for_callback = timer.clone();
    let app_weak = app.as_weak();
    app.on_refresh_fullscreen_info(move || {
        let Some(app) = app_weak.upgrade() else {
            timer_for_callback.stop();
            return;
        };
        if !should_show(&app) {
            app.set_fullscreen_info_time("".into());
            app.set_fullscreen_info_battery("".into());
            app.set_fullscreen_info_battery_level(0.0);
            timer_for_callback.stop();
            return;
        }
        update(&app);

        let app_weak = app.as_weak();
        let timer_weak = Rc::downgrade(&timer_for_callback);
        timer_for_callback.start(TimerMode::Repeated, REFRESH_INTERVAL, move || {
            if let Some(app) = app_weak.upgrade()
                && should_show(&app)
            {
                update(&app);
            } else if let Some(timer) = timer_weak.upgrade() {
                timer.stop();
            }
        });
    });
    app.invoke_refresh_fullscreen_info();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_percent_displays_rounded_capacity_only_when_valid() {
        assert_eq!(battery_label(0.0).as_deref(), Some("0%"));
        assert_eq!(battery_label(48.5).as_deref(), Some("49%"));
        assert_eq!(battery_label(100.0).as_deref(), Some("100%"));
        for value in [-0.1, 100.1, f32::NAN, f32::INFINITY] {
            assert_eq!(battery_label(value), None);
        }
    }
}
