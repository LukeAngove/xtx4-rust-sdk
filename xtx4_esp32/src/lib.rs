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

use esp_backtrace as _;
use esp_hal::{
    prelude::*,
    gpio::{Input, Level, Output, Pull},
    spi,
    spi::master::Spi,
    spi::SpiMode,
};
use embassy_embedded_hal::SetConfig;

use esp_println::println;
use xtx4_platform_interface::{Buttons, Framebuffer, Buffer, Platform, FRAME_WIDTH, FRAME_HEIGHT};
use core::cell::Cell;

pub use esp_hal::entry;

// Intentionally inverted, for rotation.
const DISPLAY_WIDTH: u16  = FRAME_HEIGHT as u16;
const DISPLAY_HEIGHT: u16 = FRAME_WIDTH as u16;

const CTRL1_BYPASS_RED: u8 = 0x40;

// Screen orientation
//const DATA_ENTRY_X_DEC_Y_DEC: u8 = 0x00;
const DATA_ENTRY_X_INC_Y_DEC: u8 = 0x01;
//const DATA_ENTRY_X_DEC_Y_INC: u8 = 0x02;
//const DATA_ENTRY_X_INC_Y_INC: u8 = 0x03;

enum SSD1677Command {
    DriverOutputControl = 0x01,
    BoosterSoftStart    = 0x0C,
    DataEntryMode       = 0x11,
    SoftReset           = 0x12,
    TempSensorControl   = 0x18,
    BorderWaveform      = 0x3C,

    SetRamXRange        = 0x44,
    SetRamYRange        = 0x45,
    SetRamXCounter      = 0x4E,
    SetRamYCounter      = 0x4F,

    MasterActivation    = 0x20,
    DisplayUpdateCtrl1  = 0x21,
    DisplayUpdateCtrl2  = 0x22,

    WriteRamBw          = 0x24,
    WriteRamRed         = 0x26,

    AutoWriteBwRam      = 0x46,
    AutoWriteRedRam     = 0x47,
}

enum Color {
    Red,
    BlackWhite,
}

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

#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Rectangle {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

pub struct Esp32Platform {
    spi:          Spi<'static, esp_hal::Blocking>,
    cs:           Output<'static>,
    dc:           Output<'static>,
    rst:          Output<'static>,
    busy:         Input<'static>,
    ram_region:   Option<Rectangle>,
    is_screen_on: bool,
}

impl Esp32Platform {
    pub fn new() -> Self {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        let mut spi = Spi::new(peripherals.SPI2)
            .with_sck(peripherals.GPIO8)
            .with_mosi(peripherals.GPIO10);
        spi.set_config(&spi::master::Config {
                frequency: 40u32.MHz(),
                mode: SpiMode::Mode0,
                ..spi::master::Config::default()
            }).unwrap();

        let cs   = Output::new(peripherals.GPIO21, Level::High);
        let dc   = Output::new(peripherals.GPIO4,  Level::High);
        let rst  = Output::new(peripherals.GPIO5,  Level::High);
        let busy = Input::new(peripherals.GPIO6,   Pull::None);

        let mut platform = Self {
            spi,
            cs,
            dc,
            rst,
            busy,
            ram_region: None,
            is_screen_on: false,
        };

        platform.reset_display();
        platform.init_controller();
        platform
    }

    fn full_display_rect(&self) -> Rectangle {
        Rectangle {
            x: 0,
            y: 0,
            w: DISPLAY_WIDTH,
            h: DISPLAY_HEIGHT,
        }
    }

    fn reset_display(&mut self) {
        self.rst.set_high();
        self.sleep_ms(20u32);
        self.rst.set_low();
        self.sleep_ms(2u32);
        self.rst.set_high();
        self.sleep_ms(20u32);
    }

    fn send_command(&mut self, cmd: SSD1677Command) {
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write_bytes(&[cmd as u8]).unwrap();
        self.cs.set_high();
    }

    fn send_byte(&mut self, data: u8) {
        self.dc.set_high();
        self.cs.set_low();
        self.spi.write_bytes(&[data]).unwrap();
        self.cs.set_high();
    }

    fn send_data(&mut self, data: &Cell<[u8]>) {
        self.dc.set_high();
        self.cs.set_low();

        // SAFETY: read-only, fixed lifetime use
        let data: &[u8] = unsafe { &*(data.as_ptr()) };
        self.spi.write_bytes(data).unwrap();

        self.cs.set_high();
    }

    fn wait_while_busy(&mut self, comment: &str) {
        let mut timeout = 10_000u32;
        while self.busy.is_high() {
            self.sleep_ms(1u32);
            timeout -= 1;
            if timeout == 0 {
                println!("Timeout waiting for busy: {}", comment);
                return;
            }
        }
        println!("Ready: {}", comment);
    }

    fn write_region(&mut self, color: Color, buffer: &Buffer, rect: &Rectangle) {
        let Rectangle{x, y, w,h} = rect;
        let bytes_to_write = ((*w as usize)*(*h as usize)/8);

        if bytes_to_write != buffer.as_slice_of_cells().len() {
            panic!("Incorrect size for region write! Expected: {}, got: {} ({},{}) at ({}, {})", buffer.as_slice_of_cells().len(), bytes_to_write, w, h, x, y);
        }

        if self.ram_region != Some(*rect) {
            self.set_ram_area(rect);
        }

        let command = match color {
            Color::Red => SSD1677Command::WriteRamRed,
            Color::BlackWhite => SSD1677Command::WriteRamBw,
        };

        self.send_command(command);
        self.send_data(buffer);
    }

    fn set_ram_area(&mut self, region: &Rectangle) {
        // Don't bother setting region if it's already set.
        let region : Rectangle = region.clone();
        if self.ram_region == Some(region) {
            return;
        }

        // Set to 'None' during processing.
        // We should never race, but it's better practice
        // than ignoring it.
        self.ram_region = None;
        let Rectangle {x,y,w,h} = region;

        let y = DISPLAY_HEIGHT - y - h; // reverse Y for this display

        self.send_command(SSD1677Command::DataEntryMode);
        self.send_byte(DATA_ENTRY_X_INC_Y_DEC);

        self.send_command(SSD1677Command::SetRamXRange);
        self.send_byte((x % 256) as u8);
        self.send_byte((x / 256) as u8);
        self.send_byte(((x + w - 1) % 256) as u8);
        self.send_byte(((x + w - 1) / 256) as u8);

        self.send_command(SSD1677Command::SetRamYRange);
        self.send_byte(((y + h - 1) % 256) as u8);
        self.send_byte(((y + h - 1) / 256) as u8);
        self.send_byte((y % 256) as u8);
        self.send_byte((y / 256) as u8);

        self.send_command(SSD1677Command::SetRamXCounter);
        self.send_byte((x % 256) as u8);
        self.send_byte((x / 256) as u8);

        self.send_command(SSD1677Command::SetRamYCounter);
        self.send_byte(((y + h - 1) % 256) as u8);
        self.send_byte(((y + h - 1) / 256) as u8);

        self.ram_region = Some(region);
    }

    fn init_controller(&mut self) {
        println!("Initializing SSD1677...");

        self.send_command(SSD1677Command::SoftReset);
        self.wait_while_busy("soft reset");

        self.send_command(SSD1677Command::TempSensorControl);
        self.send_byte(0x80); // internal temp sensor

        self.send_command(SSD1677Command::BoosterSoftStart);
        let command = Cell::new([0xAEu8, 0xC7, 0xC3, 0xC0, 0x40]);
        self.send_data(&command);

        self.send_command(SSD1677Command::DriverOutputControl);
        self.send_byte(((DISPLAY_HEIGHT - 1) % 256) as u8);
        self.send_byte(((DISPLAY_HEIGHT - 1) / 256) as u8);
        self.send_byte(0x02); // SM=1

        self.send_command(SSD1677Command::BorderWaveform);
        self.send_byte(0x01);

        let full_screen = self.full_display_rect();

        self.set_ram_area(&full_screen);

        self.send_command(SSD1677Command::AutoWriteBwRam);
        self.send_byte(0xF7);
        self.wait_while_busy("auto write BW RAM");

        self.send_command(SSD1677Command::AutoWriteRedRam);
        self.send_byte(0xF7);
        self.wait_while_busy("auto write RED RAM");

        println!("SSD1677 ready");
    }

    fn refresh_full(&mut self) {
        self.send_command(SSD1677Command::DisplayUpdateCtrl1);
        self.send_byte(CTRL1_BYPASS_RED);

        let mut mode = 0x34u8;
        if !self.is_screen_on {
            self.is_screen_on = true;
            mode |= 0xC0; // CLOCK_ON + ANALOG_ON
        }

        self.send_command(SSD1677Command::DisplayUpdateCtrl2);
        self.send_byte(mode);
        self.send_command(SSD1677Command::MasterActivation);
        self.wait_while_busy("full refresh");
    }

    //fn write_ram_buffer(&mut self, ramBuffer: u8, data: Buffer, size: usize) {
    //  let bufferName = if (ramBuffer == SSD1677Command::WriteRamBw) { "BW" } else { "RED" };
    //  println!("Writing frame buffer to {} RAM ({} bytes)...\n", bufferName, size);

    //  sendCommand(ramBuffer);
    //  sendData(data, size);

    //  println!("{} RAM write complete ({} ms)\n", bufferName, duration);
    //}

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
        self.write_region(Color::BlackWhite, &rotated, &full_screen);
        self.write_region(Color::Red, &rotated, &full_screen);
        self.refresh_full();
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
        esp_hal::delay::Delay::new().delay_millis(ms);
    }

    fn log(&mut self, msg: &str) {
        println!("{}", msg);
    }

    fn power_off(&mut self) {
        todo!()
    }
}
