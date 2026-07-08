#![no_std]

// DisplayController trait and Display wrapper.
// Coordinates: user space. The controller handles any internal transform.

use xtx4_platform_interface::{Buffer, Rectangle};

/// Refresh mode for non-blocking updates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    Full,
    Fast,
}

/// Low-level display controller. Works in whatever coordinate space
/// the caller uses — the controller handles internal transforms.
pub trait DisplayController {
    /// Begin a refresh cycle.
    fn start_update(&mut self, fb: &Buffer, rect: &Rectangle, mode: UpdateMode) -> bool;

    /// Complete a refresh cycle after BUSY goes low.
    fn finish_update(&mut self, fb: &Buffer, rect: &Rectangle, mode: UpdateMode);

    /// Returns true while the controller is driving the panel (BUSY).
    fn is_busy(&self) -> bool;

    /// Block until BUSY goes low, with a timeout.
    fn wait_while_busy(&mut self);

    /// Put the controller into deep-sleep / low-power mode.
    fn sleep(&mut self);

    /// Full re-initialization after waking from sleep.
    /// If `clear_display` is true, clear to white.
    fn wake(&mut self, clear_display: bool);

    /// Returns true if the controller is in sleep mode.
    fn is_asleep(&self) -> bool;
}

/// Holds a framebuffer borrow across a refresh cycle.
/// On drop (or explicit `wait`), blocks until the controller finishes,
/// then calls `finish_update` for the second-phase write.
pub struct UpdateGuard<'a, D: DisplayController> {
    display: &'a mut Display<D>,
    fb: &'a Buffer,
    rect: Rectangle,
    mode: UpdateMode,
}

impl<D: DisplayController> UpdateGuard<'_, D> {
    /// Block until refresh completes, then finish (second-phase write).
    pub fn wait(self) {
        self.display.controller.wait_while_busy();
        self.display
            .controller
            .finish_update(self.fb, &self.rect, self.mode);
    }

    pub fn is_busy(&self) -> bool {
        self.display.controller.is_busy()
    }
}

impl<D: DisplayController> Drop for UpdateGuard<'_, D> {
    fn drop(&mut self) {
        self.display.controller.wait_while_busy();
        self.display
            .controller
            .finish_update(self.fb, &self.rect, self.mode);
    }
}

// ── Display wrapper ──────────────────────────────────────────────────────

/// High-level display wrapper providing non-blocking updates and
/// blocking convenience methods. Delegates all work to the controller.
pub struct Display<D: DisplayController> {
    pub controller: D,
}

impl<D: DisplayController> Display<D> {
    pub fn new(controller: D) -> Self {
        Self { controller }
    }

    /// Start a display update. Returns a guard that holds the framebuffer
    /// across the refresh and auto-completes the second-phase write.
    pub fn update<'a>(
        &'a mut self,
        fb: &'a Buffer,
        rect: &Rectangle,
        mode: UpdateMode,
    ) -> Option<UpdateGuard<'a, D>> {
        if self.controller.start_update(fb, rect, mode) {
            Some(UpdateGuard {
                display: self,
                fb,
                rect: *rect,
                mode,
            })
        } else {
            None
        }
    }

    pub fn is_busy(&self) -> bool {
        self.controller.is_busy()
    }

    pub fn wait(&mut self) {
        self.controller.wait_while_busy();
    }

    // ── Blocking convenience wrappers ─────────────────────────────────

    pub fn flush_full(&mut self, fb: &Buffer, rect: &Rectangle) {
        if let Some(g) = self.update(fb, rect, UpdateMode::Full) {
            g.wait();
        }
    }

    pub fn flush_partial(&mut self, fb: &Buffer, rect: &Rectangle) {
        if let Some(g) = self.update(fb, rect, UpdateMode::Fast) {
            g.wait();
        }
    }

    pub fn fast_full(&mut self, fb: &Buffer, rect: &Rectangle) {
        if let Some(g) = self.update(fb, rect, UpdateMode::Fast) {
            g.wait();
        }
    }

    pub fn sleep(&mut self) {
        self.controller.sleep();
    }

    pub fn wake(&mut self, clear_display: bool) {
        self.controller.wake(clear_display);
    }

    pub fn is_asleep(&self) -> bool {
        self.controller.is_asleep()
    }
}
