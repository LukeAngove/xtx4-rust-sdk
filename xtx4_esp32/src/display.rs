use core::cell::Cell;

use xtx4_platform_interface::{Buffer, Rectangle};
use ssd1677::{SSD1677, Color, DriverOutputControlMode, DataEntryMode, Range};
use ssd1677::DisplayTransport;

pub struct Display<T: DisplayTransport> {
    pub controller: SSD1677<T>,
    width: u16,
    height: u16,
    ram_region:   Option<Rectangle>,
}

impl<T: DisplayTransport> Display<T> {
    pub fn new(transport: T, width: u16, height: u16) -> Self {
        let controller = SSD1677::new(transport);
        let mut res = Self {controller , width, height, ram_region: None};
        res.init();
        res
    }

    pub fn full_display_rect(&self) -> Rectangle {
        Rectangle {
            x: 0,
            y: 0,
            w: self.width,
            h: self.height,
        }
    }


    fn init(&mut self) {
        self.controller.reset();

        self.controller.soft_reset();

        self.controller.set_temp_sensor(0x80); // internal temp sensor

        // Level 1 booster soft start. Level 2 is the only other supported,
        // and that is only 0x80 as the last byte.
        let command = Cell::new([0xAEu8, 0xC7, 0xC3, 0xC0, 0x40]);
        self.controller.booster_soft_start(&command);

        self.controller.driver_output_control(self.height, DriverOutputControlMode::SM);

        self.controller.set_border_waveform(0x01);

        let full_screen = self.full_display_rect();

        self.set_ram_area(&full_screen);

        self.controller.auto_write_ram(Color::BlackWhite, 0xF7);
        self.controller.auto_write_ram(Color::Red, 0xF7);
    }

    pub fn set_ram_area(&mut self, region: &Rectangle) {
        // Don't bother setting region if it's already set.
        //if self.ram_region == Some(*region) {
        //    return;
        //}

        // Set to 'None' during processing.
        // We should never race, but it's better practice
        // than ignoring it.
        self.ram_region = None;

        self.set_ram_area_intern(region);

        self.ram_region = Some(region.clone());
    }

    fn set_ram_area_intern(&mut self, region: &Rectangle) {
        // Don't bother setting region if it's already set.
        let Rectangle {x,y,w,h} = *region;

        let y = self.height - y - h; // reverse Y for this Display

        // Flipping these reflects the display in that axis.
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

        // We might need to always reset the counters, even if the region is the same.
        self.controller.set_ram_counter(Range::X, x_ends.0);
        self.controller.set_ram_counter(Range::Y, y_ends.0);
    }

    pub fn read_buffer(&mut self, color: Color) {
        self.set_ram_area(&self.full_display_rect());
        self.controller.read_ram(color);
    }

    /// Rotate a portrait frame to landscape coordinates for the display controller.
    /// This is the shared rotation used by both hardware and emulated platforms.
    pub fn rotate_rect(&self, rect: &Rectangle) -> Rectangle {
        let Rectangle { x, y, w, h } = *rect;
        Rectangle {
            x: y,
            y: self.height - x - w,
            w: h,
            h: w,
        }
    }

    /// Full partial-update: write BW → refresh → write Red (for next cycle).
    pub fn flush_partial(&mut self, fb: &Buffer, frame: &Rectangle) {
        let rotated = self.rotate_rect(frame);
        self.write_region(Color::BlackWhite, fb, &rotated);
        self.refresh_partial();
        self.write_region(Color::Red, fb, &rotated);
    }

    /// Full display flush (full refresh): write BW + Red → refresh_full.
    pub fn flush_full(&mut self, fb: &Buffer) {
        let full = self.full_display_rect();
        self.write_region(Color::BlackWhite, fb, &full);
        self.write_region(Color::Red, fb, &full);
        self.refresh_full();
    }

    /// Fast partial update of full screen: write BW → refresh_partial → write Red.
    pub fn fast_full(&mut self, fb: &Buffer) {
        let full = self.full_display_rect();
        self.write_region(Color::BlackWhite, fb, &full);
        self.refresh_partial();
        self.write_region(Color::Red, fb, &full);
    }

    pub fn write_region(&mut self, color: Color, buffer: &Buffer, rect: &Rectangle) {
        let Rectangle{x, y, w, h} = rect;
        let bytes_to_write = (*w as usize)*(*h as usize)/8;

        if bytes_to_write != buffer.as_slice_of_cells().len() {
            panic!("Incorrect size for region write! Expected: {}, got: {} ({},{}) at ({}, {})", buffer.as_slice_of_cells().len(), bytes_to_write, w, h, x, y);
        }

        //if self.ram_region != Some(*rect) {
            self.set_ram_area(rect);
        //}

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
