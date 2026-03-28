#![no_std]

// This file is a direct port of https://github.com/open-x4-epaper/community-sdk/blob/9f76376a5cc7894cff9ca87bbdd34dab715d8a59/libs/display/EInkDisplay/src/EInkDisplay.cpp

// ESP32-C3 hardware backend — enabled with feature = "esp32".
//
// Hardware pin reference (from community SDK):
//   SPI bus : SCLK=8, MOSI=10
//   EPD CS  : GPIO 21
//   EPD DC  : GPIO 4   (command/data select)
//   EPD RST : GPIO 5
//   EPD BUSY: GPIO 6   (active-high = busy)
//   SD CS   : GPIO 12  (shares SPI bus)
//   Buttons : resistor ladder on ADC (check SDK for voltage thresholds)
//   Battery : GPIO 0, voltage divider — ADC read = 0.5 × actual voltage

mod sleep;
mod ssd1677;
mod display;
mod rectangle;

use esp_backtrace as _;

use esp_println::println;
use xtx4_platform_interface::{Buttons, Framebuffer, Buffer, Platform, FRAME_WIDTH, FRAME_HEIGHT};
use core::cell::Cell;

use crate::ssd1677::{SSD1677, SSD1677Builder, Color};
use crate::display::Display;
use crate::rectangle::Rectangle;
use crate::sleep::sleep_ms;

// Intentionally inverted, for rotation.
const DISPLAY_WIDTH: u16  = FRAME_HEIGHT as u16;
const DISPLAY_HEIGHT: u16 = FRAME_WIDTH as u16;

fn rotate_90(fb: &Framebuffer) -> Framebuffer {
    // Input:  landscape 800w x 480h, row-major, 1bpp
    // Output: portrait  480w x 800h, row-major, 1bpp
    let out = Framebuffer::new([0; (DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize + 7) / 8]);
    let fb = fb.as_array_of_cells();
    let out_b = out.as_array_of_cells();
    for y in 0..FRAME_HEIGHT as usize {
        for x in 0..FRAME_WIDTH as usize {

            let src_byte = y * (FRAME_WIDTH / 8) + x / 8;
            let src_bit = 7 - (x % 8);
            let is_white = (fb[src_byte].get() >> src_bit) & 1;

            let dst_x = y;
            let dst_y = (DISPLAY_HEIGHT as usize - 1) - x;
            let dst_byte = dst_y * (DISPLAY_WIDTH as usize / 8) + dst_x / 8;
            let dst_bit = 7 - (dst_x % 8);

            if is_white == 1 {
                out_b[dst_byte].set(out_b[dst_byte].get() | 1 << dst_bit);
            }
        }
    }
    out
}

pub struct Esp32Platform {
    //display:      SSD1677,
    display:      Display,
}

impl Esp32Platform {
    pub fn new() -> Self {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        let builder = SSD1677Builder {
            spi: peripherals.SPI2.into(),
            sck: peripherals.GPIO8.into(),
            mosi: peripherals.GPIO10.into(),
            cs: peripherals.GPIO21.into(),
            dc: peripherals.GPIO4.into(),
            rst: peripherals.GPIO5.into(),
            busy: peripherals.GPIO6.into(),
        };
        let controller = SSD1677::new(builder);
        let display = Display::new(controller, DISPLAY_WIDTH, DISPLAY_HEIGHT);

        Self {
            display,
        }
    }

    fn full_display_rect(&self) -> Rectangle {
        Rectangle {
            x: 0,
            y: 0,
            w: DISPLAY_WIDTH,
            h: DISPLAY_HEIGHT,
        }
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

impl Platform for Esp32Platform {
    fn display_flush(&mut self, fb: &Framebuffer) {

        let rotated = rotate_90(fb);

        self.log("display_flush");
        let full_screen = self.full_display_rect();
        self.display.write_region(Color::BlackWhite, &rotated, &full_screen);
        self.display.write_region(Color::Red, &rotated, &full_screen);
        self.display.refresh_full();
    }

    fn display_flush_partial(&mut self, _fb: &Cell<[u8]>, _x: u16, _y: u16, _w: u16, _h: u16) {
        todo!()
    }

    fn button_state(&mut self) -> Buttons {
        // TODO: read ADC, map voltage ranges to buttons.
        // Thresholds are in the community SDK hardware lib.
        //todo!()
        Buttons::empty()
    }

    fn now_ms(&self) -> u32 {
        esp_hal::time::now().duration_since_epoch().to_millis() as u32
    }

    fn sleep_ms(&mut self, ms: u32) {
        sleep_ms(ms);
    }

    fn log(&mut self, msg: &str) {
        println!("{}", msg);
    }

    fn power_off(&mut self) {
        todo!()
    }
}
