//! Background button reader — wraps any [`ButtonReader`] in a background
//! thread that polls at 10 ms intervals, caches the result, and tracks
//! press/release latches so short presses during display updates are not
//! missed.  Mimics the behaviour of the hardware ISR-driven reader.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use xtx4_platform_interface::Buttons;
use xtx4_buttons::ButtonReader;

pub struct BackgroundButtons<T: ButtonReader + Send + 'static> {
    shared: Arc<Mutex<State>>,
    shutdown: Arc<AtomicBool>,
    _handle: Option<JoinHandle<()>>,
    _phantom: std::marker::PhantomData<T>,
}

struct State {
    current: u8,
    pressed_latch: u8,
    released_latch: u8,
}

impl<T: ButtonReader + Send + 'static> BackgroundButtons<T> {
    /// Spawn a background thread that calls `inner.button_state()` every
    /// 10 ms, debounces via a two-sample match, and tracks press/release
    /// latches.  The thread stops when this struct is dropped.
    pub fn new(inner: T) -> Self {
        let shared = Arc::new(Mutex::new(State {
            current: 0,
            pressed_latch: 0,
            released_latch: 0,
        }));
        let shutdown = Arc::new(AtomicBool::new(false));

        let shared_clone = shared.clone();
        let shutdown_clone = shutdown.clone();
        let inner = Arc::new(Mutex::new(inner));

        let handle = thread::spawn(move || {
            let mut last_raw: u8 = 0;
            let mut db_count: u8 = 0;

            while !shutdown_clone.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(10));
                let raw = inner.lock().unwrap().button_state().bits();

                // Two-sample debounce
                if raw == last_raw {
                    db_count = db_count.saturating_add(1);
                    if db_count >= 2 {
                        let mut state = shared_clone.lock().unwrap();
                        let old = state.current;
                        state.current = raw;
                        state.pressed_latch |= raw & !old;
                        state.released_latch |= old & !raw;
                    }
                } else {
                    db_count = 0;
                }
                last_raw = raw;
            }
        });

        Self {
            shared,
            shutdown,
            _handle: Some(handle),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: ButtonReader + Send + 'static> Drop for BackgroundButtons<T> {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self._handle.take() {
            let _ = handle.join();
        }
    }
}

impl<T: ButtonReader + Send + 'static> ButtonReader for BackgroundButtons<T> {
    fn button_state(&mut self) -> Buttons {
        let mut state = self.shared.lock().unwrap();
        let current = state.current;
        let pressed = state.pressed_latch;
        let released = state.released_latch;
        state.pressed_latch = 0;
        state.released_latch = 0;
        Buttons::from_bits(current | pressed | released).unwrap_or(Buttons::empty())
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Tests
// ════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    struct SharedButtons {
        state: Arc<StdMutex<Buttons>>,
    }

    impl ButtonReader for SharedButtons {
        fn button_state(&mut self) -> Buttons {
            *self.state.lock().unwrap()
        }
    }

    impl SharedButtons {
        fn new(state: Arc<StdMutex<Buttons>>) -> Self {
            Self { state }
        }
    }

    #[test]
    fn hold_across_polls() {
        let raw = Arc::new(StdMutex::new(Buttons::empty()));
        let bg = BackgroundButtons::new(SharedButtons::new(raw.clone()));

        // Press LEFT_OUTER
        *raw.lock().unwrap() = Buttons::LEFT_OUTER;
        thread::sleep(Duration::from_millis(50));

        // Should see the press
        let r = bg.shared.lock().unwrap();
        assert_eq!(r.current, Buttons::LEFT_OUTER.bits());
        assert_ne!(r.pressed_latch & Buttons::LEFT_OUTER.bits(), 0);
    }

    #[test]
    fn tap_between_polls() {
        let raw = Arc::new(StdMutex::new(Buttons::empty()));
        let mut bg = BackgroundButtons::new(SharedButtons::new(raw.clone()));

        // Tap: press then release
        *raw.lock().unwrap() = Buttons::LEFT_OUTER;
        thread::sleep(Duration::from_millis(50));
        *raw.lock().unwrap() = Buttons::empty();
        thread::sleep(Duration::from_millis(50));

        // First poll: should see the press
        let r = bg.button_state();
        assert_ne!(r.bits() & Buttons::LEFT_OUTER.bits(), 0);

        // Second poll: nothing
        assert_eq!(bg.button_state(), Buttons::empty());
    }

    #[test]
    fn drop_stops_thread() {
        let raw = Arc::new(StdMutex::new(Buttons::empty()));
        let bg = BackgroundButtons::new(SharedButtons::new(raw.clone()));
        drop(bg);
        // Drop should complete without hanging.
    }
}
