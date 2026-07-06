use core::cell::Cell;

use xtx4_platform_interface::{Buffer, Framebuffer, Rectangle};
use crate::{SSD1677, Color, DriverOutputControlMode, DataEntryMode, Range, DisplayTransport};

/// Refresh mode for non-blocking updates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    Full,
    Fast,
}

pub struct Display<T: DisplayTransport> {
    pub controller: SSD1677<T>,
    width_l: u16,
    height_l: u16,
    ram_region:   Option<Rectangle>,
}

/// Borrows the framebuffer across refresh cycles.
/// Automatically completes the two-phase write on drop.
pub struct UpdateGuard<'a, T: DisplayTransport> {
    display: &'a mut Display<T>,
    fb: &'a Buffer,
    rect_l: Rectangle,
    mode: UpdateMode,
}

impl<T: DisplayTransport> UpdateGuard<'_, T> {
    /// Block until BUSY low, then do the second BW write (fast mode only).
    pub fn wait(self) {
        self.display.controller.wait("fast (wait)");
        if self.mode == UpdateMode::Fast {
            self.display.write_region(Color::BlackWhite, self.fb, &self.rect_l);
        }
    }

    pub fn is_busy(&self) -> bool {
        self.display.controller.is_busy()
    }
}

impl<T: DisplayTransport> Drop for UpdateGuard<'_, T> {
    fn drop(&mut self) {
        if self.mode == UpdateMode::Fast {
            self.display.controller.wait("fast (drop)");
            self.display.write_region(Color::BlackWhite, self.fb, &self.rect_l);
        }
    }
}

// ── Coordinate space helpers ────────────────────────────────────────────

/// Portrait dimensions (application / framebuffer space).
pub const PORTRAIT_W: u16 = 480;
pub const PORTRAIT_H: u16 = 800;

/// Rotate a portrait rect to landscape coordinates for the SSD1677 controller.
fn portrait_to_landscape(portrait: &Rectangle, height_l: u16) -> Rectangle {
    let Rectangle { x, y, w, h } = *portrait;
    Rectangle {
        x: y,
        y: height_l - x - w,
        w: h,
        h: w,
    }
}

impl<T: DisplayTransport> Display<T> {
    pub fn new(transport: T, width_l: u16, height_l: u16) -> Self {
        let controller = SSD1677::new(transport);
        let mut res = Self { controller, width_l, height_l, ram_region: None };
        res.init();
        res
    }

    /// Full display rect in landscape coordinates (controller space).
    fn full_rect_l(&self) -> Rectangle {
        Rectangle { x: 0, y: 0, w: self.width_l, h: self.height_l }
    }

    fn init(&mut self) {
        self.controller.reset();
        self.controller.soft_reset();
        self.controller.set_temp_sensor(0x80); // internal temp sensor

        let command = Cell::new([0xAEu8, 0xC7, 0xC3, 0xC0, 0x40]);
        self.controller.booster_soft_start(&command);

        self.controller.driver_output_control(self.height_l, DriverOutputControlMode::SM);
        self.controller.set_border_waveform(0x01);

        let full = self.full_rect_l();
        self.set_ram_area(&full);

        self.controller.auto_write_ram(Color::BlackWhite, 0xF7);
        self.controller.auto_write_ram(Color::Red, 0xF7);
    }

    pub fn set_ram_area(&mut self, region_l: &Rectangle) {
        self.ram_region = None;
        self.set_ram_area_intern(region_l);
        self.ram_region = Some(region_l.clone());
    }

    fn set_ram_area_intern(&mut self, region_l: &Rectangle) {
        let Rectangle {x,y,w,h} = *region_l;

        let y = self.height_l - y - h;

        let x_dir = DataEntryMode::Increase;
        let y_dir = DataEntryMode::Decrease;

        let x_ends = match x_dir {
            DataEntryMode::Increase => (x, x+w-1),
            DataEntryMode::Decrease => (x+w-1, x),
        };

        let y_ends = match y_dir {
            DataEntryMode::Increase => (y, y+h-1),
            DataEntryMode::Decrease => (y+h-1, y),
        };

        self.controller.set_data_mode(x_dir, y_dir);
        self.controller.set_ram_range(Range::X, x_ends.0, x_ends.1);
        self.controller.set_ram_range(Range::Y, y_ends.0, y_ends.1);
        self.controller.set_ram_counter(Range::X, x_ends.0);
        self.controller.set_ram_counter(Range::Y, y_ends.0);
    }

    pub fn read_buffer(&mut self, color: Color) {
        let full = self.full_rect_l();
        self.set_ram_area(&full);
        self.controller.read_ram(color);
    }

    // ── Non-blocking API (portrait space) ────────────────────────────

    /// Start a display update. `rect_p` is in portrait coordinates.
    /// Returns a guard that holds the framebuffer until the update completes.
    ///
    /// **Full:**  write BW + Red → trigger full refresh → return
    /// **Fast:**  write BW → trigger partial → second write on wait() or drop
    pub fn update<'a>(&'a mut self, fb: &'a Buffer, rect_p: &Rectangle, mode: UpdateMode) -> UpdateGuard<'a, T> {
        let rect_l = portrait_to_landscape(rect_p, self.height_l);

        // Wait for previous refresh to finish before writing RAM.
        self.controller.wait("pre-update");

        match mode {
            UpdateMode::Full => {
                self.write_region(Color::BlackWhite, fb, &rect_l);
                self.write_region(Color::Red, fb, &rect_l);
                self.controller.trigger_full();
            } 
            UpdateMode::Fast => {
                self.write_region(Color::BlackWhite, fb, &rect_l);
                self.controller.trigger_partial();
            }
        }

        UpdateGuard { display: self, fb, rect_l, mode }
    }

    /// Returns true if the display controller is busy refreshing.
    pub fn is_busy(&self) -> bool {
        self.controller.is_busy()
    }

    /// Block until the display controller finishes its current refresh.
    pub fn wait(&mut self) {
        self.controller.wait("wait");
    }

    // ── Blocking convenience wrappers (all portrait space) ────────────

    pub fn flush_full(&mut self, fb: &Buffer) {
        let full_p = Rectangle { x: 0, y: 0, w: PORTRAIT_W, h: PORTRAIT_H };
        let guard = self.update(fb, &full_p, UpdateMode::Full);
        guard.wait();
    }

    pub fn flush_partial(&mut self, fb: &Buffer, frame_p: &Rectangle) {
        let guard = self.update(fb, frame_p, UpdateMode::Fast);
        guard.wait();
    }

    pub fn fast_full(&mut self, fb: &Framebuffer) {
        let full_p = Rectangle { x: 0, y: 0, w: PORTRAIT_W, h: PORTRAIT_H };
        let guard = self.update(fb, &full_p, UpdateMode::Fast);
        guard.wait();
    }

    // ── Raw I/O (landscape space) ──────────────────────────────────────

    pub fn write_region(&mut self, color: Color, buffer: &Buffer, rect_l: &Rectangle) {
        let Rectangle{x, y, w, h} = rect_l;
        let bytes_to_write = (*w as usize)*(*h as usize)/8;

        if bytes_to_write != buffer.as_slice_of_cells().len() {
            panic!("Incorrect size for region write! Expected: {}, got: {} ({},{}) at ({}, {})", buffer.as_slice_of_cells().len(), bytes_to_write, w, h, x, y);
        }

        self.set_ram_area(rect_l);
        self.controller.write_ram(color, buffer);
    }

    pub fn refresh_full(&mut self) {
        self.controller.refresh_full();
    }

    pub fn refresh_partial(&mut self) {
        self.controller.refresh_partial();
    }

    pub fn sleep(&mut self) {
        self.controller.screen_sleep();
    }
}
