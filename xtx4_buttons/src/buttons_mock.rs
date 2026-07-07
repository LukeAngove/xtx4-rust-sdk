// Programmable mock button reader for integration testing (x86_64 only).
// Uses a global event queue so tests can push events before constructing the platform.

use std::sync::Mutex;
use xtx4_platform_interface::Buttons;
use crate::ButtonReader;

static EVENT_QUEUE: Mutex<Vec<Buttons>> = Mutex::new(Vec::new());

pub struct MockButtons;

impl MockButtons {
    /// XtX4::new() reads button_state once during construction,
    /// so pre-queue an empty read to absorb that initial poll.
    pub fn new() -> Self {
        EVENT_QUEUE.lock().unwrap().insert(0, Buttons::empty());
        MockButtons
    }
    pub fn queue(buttons: Buttons) {
        EVENT_QUEUE.lock().unwrap().push(buttons);
    }

    pub fn clear() {
        EVENT_QUEUE.lock().unwrap().clear();
    }
}

impl ButtonReader for MockButtons {
    fn button_state(&mut self) -> Buttons {
        let mut q = EVENT_QUEUE.lock().unwrap();
        if q.is_empty() { Buttons::empty() } else { q.remove(0) }
    }
}