//! Apply the saved backdrop and follow changes to Windows material policy.
use super::super::materials;
use crate::NativeVideoHost;
use slint::{
    ComponentHandle, Timer,
    winit_030::{WinitWindowAccessor, winit::window::Window},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

pub(super) fn install(host: &NativeVideoHost, window: &Window, timer: &Timer) {
    let initial_material_policy = materials::native_mica_enabled();
    let mica_attribute_applied =
        match initial_material_policy.then(|| materials::apply_main_window_mica(window)) {
            Some(Ok(())) => true,
            Some(Err(error)) => {
                crate::diagnostics::warn!(
                    "Windows system backdrop unavailable; keeping the renderer fallback: {error}"
                );
                false
            }
            None => false,
        };
    host.set_mica_backdrop_enabled(mica_attribute_applied);
    let material_host = host.as_weak();
    let last_material_policy = Rc::new(Cell::new(initial_material_policy));
    let applied_material_policy = Rc::new(Cell::new(if initial_material_policy {
        mica_attribute_applied.then_some(true)
    } else {
        Some(false)
    }));
    let material_retry_delay = Rc::new(Cell::new(Duration::from_millis(500)));
    let material_retry_after = Rc::new(Cell::new(Instant::now()));
    let material_last_error = Rc::new(RefCell::new(None::<String>));
    let timer_material_host = material_host.clone();
    let timer_last_material_policy = last_material_policy.clone();
    let timer_applied_material_policy = applied_material_policy.clone();
    let timer_material_retry_delay = material_retry_delay.clone();
    let timer_material_retry_after = material_retry_after.clone();
    let timer_material_last_error = material_last_error.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let policy_enabled = materials::native_mica_enabled();
            let policy_changed = timer_last_material_policy.get() != policy_enabled;
            if policy_changed {
                timer_last_material_policy.set(policy_enabled);
                timer_material_retry_delay.set(Duration::from_millis(500));
                timer_material_retry_after.set(Instant::now());
                *timer_material_last_error.borrow_mut() = None;
            }
            let Some(host) = timer_material_host.upgrade() else {
                return;
            };
            if timer_applied_material_policy.get() == Some(policy_enabled)
                && host.get_mica_backdrop_enabled() == policy_enabled
            {
                return;
            }
            if Instant::now() < timer_material_retry_after.get() {
                return;
            }
            let mut update_result = None;
            host.window().with_winit_window(|window| {
                update_result = Some(if policy_enabled {
                    materials::apply_main_window_mica(window)
                } else {
                    materials::clear_main_window_backdrop(window)
                });
            });
            match update_result {
                Some(Ok(())) => {
                    timer_applied_material_policy.set(Some(policy_enabled));
                    host.set_mica_backdrop_enabled(policy_enabled);
                    timer_material_retry_delay.set(Duration::from_millis(500));
                    *timer_material_last_error.borrow_mut() = None;
                }
                Some(Err(error)) => {
                    timer_applied_material_policy.set(None);
                    if !policy_enabled {
                        host.set_mica_backdrop_enabled(false);
                    }
                    let mut last_error = timer_material_last_error.borrow_mut();
                    if last_error.as_deref() != Some(error.as_str()) {
                        crate::diagnostics::warn!(
                            "Could not update the Windows system backdrop policy: {error}"
                        );
                        *last_error = Some(error);
                    }
                    let retry_delay = timer_material_retry_delay.get();
                    timer_material_retry_after.set(Instant::now() + retry_delay);
                    timer_material_retry_delay
                        .set(retry_delay.saturating_mul(2).min(Duration::from_secs(30)));
                }
                None => {
                    timer_applied_material_policy.set(None);
                    if !policy_enabled {
                        host.set_mica_backdrop_enabled(false);
                    }
                    let retry_delay = timer_material_retry_delay.get();
                    timer_material_retry_after.set(Instant::now() + retry_delay);
                    timer_material_retry_delay
                        .set(retry_delay.saturating_mul(2).min(Duration::from_secs(30)));
                }
            }
        },
    );
}
