use esp_println::println;

use core::cell::Cell;

use xtx4_platform_interface::Buffer;
use crate::ssd1677::{SSD1677, Color, DriverOutputControlMode, DataEntryMode};
use crate::rectangle::Rectangle;

pub struct Display {
    controller: SSD1677,
    width: u16,
    height: u16,
    ram_region:   Option<Rectangle>,
}

impl Display {
    pub fn new(controller: SSD1677, width: u16, height: u16) -> Self {
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
        println!("Initializing SSD1677...");
        self.controller.reset();

        self.controller.soft_reset();

        self.controller.set_temp_sensor(0x80); // internal temp sensor

        let command = Cell::new([0xAEu8, 0xC7, 0xC3, 0xC0, 0x40]);
        self.controller.booster_soft_start(&command);

        self.controller.driver_output_control(self.height, DriverOutputControlMode::SM);

        self.controller.set_border_waveform(0x01);

        let full_screen = self.full_display_rect();

        self.set_ram_area(&full_screen);

        self.controller.auto_write_ram(Color::BlackWhite, 0xF7);

        self.controller.auto_write_ram(Color::Red, 0xF7);

        println!("SSD1677 ready");
    }

    pub fn set_ram_area(&mut self, region: &Rectangle) {
        // Don't bother setting region if it's already set.
        if self.ram_region == Some(*region) {
            return;
        }

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

        let y = self.height - y - h; // reverse Y for this display

        self.controller.set_data_mode(DataEntryMode::Increase, DataEntryMode::Decrease);
        self.controller.set_ram_x_range(x, w);
        self.controller.set_ram_y_range(y, h);
        self.controller.set_ram_x_counter(x);
        self.controller.set_ram_y_counter(y, h);
    }

    pub fn write_region(&mut self, color: Color, buffer: &Buffer, rect: &Rectangle) {
        let Rectangle{x, y, w, h} = rect;
        let bytes_to_write = (*w as usize)*(*h as usize)/8;

        if bytes_to_write != buffer.as_slice_of_cells().len() {
            panic!("Incorrect size for region write! Expected: {}, got: {} ({},{}) at ({}, {})", buffer.as_slice_of_cells().len(), bytes_to_write, w, h, x, y);
        }

        if self.ram_region != Some(*rect) {
            self.set_ram_area(rect);
        }

        self.controller.write_ram(color, buffer);
    }

    pub fn refresh_full(&mut self) {
        self.controller.refresh_full();
    }
}
