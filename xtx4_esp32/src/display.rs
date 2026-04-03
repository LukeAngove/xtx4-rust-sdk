use esp_println::println;

use core::cell::Cell;

use xtx4_platform_interface::{Buffer, Rectangle};
use crate::ssd1677::{SSD1677, Color, DriverOutputControlMode, DataEntryMode, Range};

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

    /*fn display_window(&mut self, fb: &Framebuffer, x: u16, y: u16, w: u16, h: u16, turn_off_screen: bool) {
      println!("Displaying window at ({},{}) size ({}x{})", x, y, w, h);

      // Validate bounds
      if (x + w > DISPLAY_WIDTH) || (y + h > DISPLAY_HEIGHT) {
        println!("ERROR: Window bounds exceed display dimensions!");
        return;
      }

      // Validate byte alignment
      if (x % 8 != 0) || (w % 8 != 0) {
        println!("ERROR: Window x and width must be byte-aligned (multiples of 8)!");
        return;
      }

      // displayWindow is not supported while the rest of the screen has grayscale content, revert it
      //if (inGrayscaleMode) {
      //  inGrayscaleMode = false;
      //  grayscaleRevert();
      //}

      // Calculate window buffer size
      let window_width_bytes = w / 8;
      let window_buffer_size = window_width_bytes * h;

      println!("Window buffer size: {} bytes ({} x {} pixels)", window_buffer_size, w, h);

      // Allocate temporary buffer on stack
      let window_buffer = Cell::new([0u8; window_buffer_size]);

      // Extract window region from frame buffer
      for row in 0..h {
        let src_y = y + row;
        let src_offset = src_y * (DISPLAY_WIDTH / 8) + (x / 8);
        let dst_offset = row * window_width_bytes;
        //memcpy(window_buffer[dstOffset], frameBuffer[srcOffset], window_width_bytes);
      }

      let window_rect = Rectangle{x, y, w, h};
      self.write_region(Color::BlackWhite, window_buffer, &window_rect);

      let single_buffer_mode = false;

      if !single_buffer_mode {
          // Dual buffer: Extract window from frameBufferActive (previous frame)
          let previous_window_buffer = Cell::new([0u8; window_buffer_size]);

          for row in 0..h {
            let src_y = y + row;
            let src_offset = src_y * (DISPLAY_WIDTH / 8) + (x / 8);
            let dst_offset = row * window_width_bytes;
 
            //memcpy(previous_window_buffer[dstOffset], frame_buffer_active[srcOffset], window_width_bytes);
          }

          self.write_region(Color::Red, previous_window_buffer, &window_rect);
      }

      // Perform fast refresh
      //refreshDisplay(FAST_REFRESH, turnOffScreen);

      if single_buffer_mode {
          // Post-refresh: Sync RED RAM with current window (for next fast refresh)
          self.write_region(Color::Red, window_buffer, &window_rect);
      }

      println!("Window display complete");
    }*/

}
