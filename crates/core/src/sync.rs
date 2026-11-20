//! Recover mutexes whose protected values stay valid after an interrupted update.
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks plain data that remains valid after unwinding through its guard.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_mutex_retains_its_data() {
        let data = Mutex::new(vec![1]);
        let result = std::panic::catch_unwind(|| {
            let mut items = lock(&data);
            items.push(2);
            panic!("interrupted update");
        });
        assert!(result.is_err());
        assert!(data.is_poisoned());
        assert_eq!(&*lock(&data), &[1, 2]);
    }
}
