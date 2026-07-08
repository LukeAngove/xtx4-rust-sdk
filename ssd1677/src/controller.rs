use core::cell::Cell;

use xtx4_display::{DisplayController, UpdateMode};
use xtx4_platform_interface::{Buffer, Rectangle};
use crate::{
    ssd1677::SSD1677, Color, DataEntryMode, DisplayInterface, DriverOutputControlMode,
    Range,
};

fn portrait_to_landscape(portrait: &Rectangle, height_l: u16) -> Rectangle {
    let Rectangle { x, y, w, h } = *portrait;
    Rectangle {
        x: y,
        y: height_l - x - w,
        w: h,
        h: w,
    }
}

/// SSD1677 display controller implementing `DisplayController`.
/// Accepts user-space (portrait) coordinates and handles rotation
/// to landscape internally.
pub struct Ssd1677Controller<T: DisplayInterface> {
    chip: SSD1677<T>,
    width_l: u16,
    height_l: u16,
    ram_region: Option<Rectangle>,
}

impl<T: DisplayInterface> Ssd1677Controller<T> {
    pub fn new(transport: T, width_l: u16, height_l: u16) -> Self {
        let chip = SSD1677::new(transport);
        let mut res = Self {
            chip,
            width_l,
            height_l,
            ram_region: None,
        };
        res.init();
        res
    }

    fn full_rect_l(&self) -> Rectangle {
        Rectangle {
            x: 0,
            y: 0,
            w: self.width_l,
            h: self.height_l,
        }
    }

    fn init(&mut self) {
        self.wake(true);
    }

    pub fn set_ram_area(&mut self, region_l: &Rectangle) {
        self.ram_region = None;
        self.set_ram_area_intern(region_l);
        self.ram_region = Some(*region_l);
    }

    fn set_ram_area_intern(&mut self, region_l: &Rectangle) {
        let Rectangle { x, y, w, h } = *region_l;

        let y = self.height_l - y - h;

        let x_dir = DataEntryMode::Increase;
        let y_dir = DataEntryMode::Decrease;

        let x_ends = match x_dir {
            DataEntryMode::Increase => (x, x + w - 1),
            DataEntryMode::Decrease => (x + w - 1, x),
        };

        let y_ends = match y_dir {
            DataEntryMode::Increase => (y, y + h - 1),
            DataEntryMode::Decrease => (y + h - 1, y),
        };

        self.chip.set_data_mode(x_dir, y_dir);
        self.chip.set_ram_range(Range::X, x_ends.0, x_ends.1);
        self.chip.set_ram_range(Range::Y, y_ends.0, y_ends.1);
        self.chip.set_ram_counter(Range::X, x_ends.0);
        self.chip.set_ram_counter(Range::Y, y_ends.0);
    }

    fn write_region(&mut self, color: Color, buffer: &Buffer, rect_l: &Rectangle) {
        let Rectangle { x: _, y: _, w, h } = rect_l;
        let bytes_to_write = (*w as usize) * (*h as usize) / 8;

        if bytes_to_write != buffer.as_slice_of_cells().len() {
            panic!(
                "Incorrect size for region write! Expected: {}, got: {} ({},{}) at ({}, {})",
                buffer.as_slice_of_cells().len(),
                bytes_to_write,
                w,
                h,
                rect_l.x,
                rect_l.y,
            );
        }

        self.set_ram_area(rect_l);
        self.chip.write_ram(color, buffer);
    }
}

impl<T: DisplayInterface> DisplayController for Ssd1677Controller<T> {
    fn wake(&mut self, clear_display: bool) {
        self.chip.reset();
        self.chip.soft_reset();
        self.chip.set_temp_sensor(0x80);

        let command = Cell::new([0xAEu8, 0xC7, 0xC3, 0xC0, 0x40]);
        self.chip.booster_soft_start(&command);

        self.chip
            .driver_output_control(self.height_l, DriverOutputControlMode::SM);
        self.chip.set_border_waveform(0x01);

        if clear_display {
            let full = self.full_rect_l();
            self.set_ram_area(&full);
            self.chip.auto_write_ram(Color::BlackWhite, 0xFF);
            self.chip.auto_write_ram(Color::Red, 0xFF);
        }

        self.chip.trigger(true);
    }

    fn sleep(&mut self) {
        self.chip.screen_sleep();
    }

    fn is_asleep(&self) -> bool {
        !self.chip.is_screen_on()
    }

    fn start_update(&mut self, fb: &Buffer, rect_p: &Rectangle, mode: UpdateMode) -> bool {
        if self.is_asleep() {
            return false;
        }
        self.chip.verify_invariant("pre start_update");
        self.wait_while_busy();
        let rect_l = portrait_to_landscape(rect_p, self.height_l);

        match mode {
            UpdateMode::Full => {
                self.write_region(Color::BlackWhite, fb, &rect_l);
                self.write_region(Color::Red, fb, &rect_l);
                self.chip.trigger(true);
            }
            UpdateMode::Fast => {
                self.write_region(Color::BlackWhite, fb, &rect_l);
                self.chip.trigger(false);
            }
        }
        true
    }

    fn finish_update(&mut self, fb: &Buffer, rect_p: &Rectangle, mode: UpdateMode) {
        if self.is_asleep() {
            return;
        }
        if mode == UpdateMode::Fast {
            let rect_l = portrait_to_landscape(rect_p, self.height_l);
            self.write_region(Color::BlackWhite, fb, &rect_l);
        }
        self.chip.verify_invariant("post finish_update");
    }

    fn is_busy(&self) -> bool {
        self.chip.is_busy()
    }

    fn wait_while_busy(&mut self) {
        self.chip.wait("controller wait_while_busy");
    }
}
