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
mod buttons;

use esp_backtrace as _;

use esp_println::println;
use xtx4_platform_interface::{Buttons, Framebuffer, Buffer, Platform, FRAME_WIDTH, FRAME_HEIGHT, Rectangle, DrawTransform};

use crate::ssd1677::{SSD1677, SSD1677Builder, Color};
use crate::display::Display;
use crate::sleep::sleep_ms;
use crate::buttons::Xtx4Buttons;

// Intentionally inverted, for rotation.
const DISPLAY_WIDTH: u16  = FRAME_HEIGHT as u16;
const DISPLAY_HEIGHT: u16 = FRAME_WIDTH as u16;

pub struct Esp32Transform;

impl DrawTransform for Esp32Transform {
    fn stride(_full_width: u16, full_height: u16) -> u16 {
        full_height
    }

    fn apply(x: u16, y: u16, width: u16, height: u16) -> Option<(u16, u16)> {
        let (p_width, p_height) = (height, width);
        let (p_x,p_y) = (y, width - 1 - x);

        if p_x < p_width && p_y < p_height {
            Some((p_x,p_y))
        } else {
            None
        }
    }
}

pub struct Esp32Platform {
    display: Display,
    buttons: Xtx4Buttons,
}

impl Esp32Platform {
    pub fn new() -> Self {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        let builder = SSD1677Builder {
            spi: peripherals.SPI2.into(),
            sck: peripherals.GPIO8.into(),
            miso: peripherals.GPIO7.into(),
            mosi: peripherals.GPIO10.into(),
            cs: peripherals.GPIO21.into(),
            dc: peripherals.GPIO4.into(),
            rst: peripherals.GPIO5.into(),
            busy: peripherals.GPIO6.into(),
        };
        let controller = SSD1677::new(builder);
        let display = Display::new(controller, DISPLAY_WIDTH, DISPLAY_HEIGHT);
        let buttons = Xtx4Buttons::new(peripherals.ADC1, peripherals.GPIO1, peripherals.GPIO2, peripherals.GPIO3.into());

        Self {
            display,
            buttons,
        }
    }
}

impl Platform for Esp32Platform {
    fn display_flush(&mut self, fb: &Framebuffer) {
        self.log("display_flush");
        let full_screen = self.display.full_display_rect();

        self.display.write_region(Color::BlackWhite, fb, &full_screen);
        self.display.write_region(Color::Red, fb, &full_screen);
        self.display.read_buffer(Color::BlackWhite);
        self.display.read_buffer(Color::Red);
        self.display.refresh_full();
        // Writing afterward only seems to be safe (no write to screen) if we disable the clock
        // and analog in refresh.
    }

    fn display_fast(&mut self, fb: &Framebuffer) {
        self.log("display_fast");
        let full_screen = self.display.full_display_rect();

        self.display.write_region(Color::BlackWhite, fb, &full_screen);
        self.display.read_buffer(Color::BlackWhite);
        self.display.read_buffer(Color::Red);
        self.display.refresh_partial();
        // Update red buffer so that it's up to date for future partial refreshes.
        self.display.write_region(Color::Red, fb, &full_screen);
    }

    fn display_flush_partial(&mut self, fb: &Buffer, frame: &Rectangle) {
        self.log("display_partial");
        let Rectangle { x, y, w, h } = *frame;

        // Need to transform for display rotation.
        let frame = &Rectangle {
            x: y,
            y: FRAME_WIDTH as u16 - x - w,
            w: h,
            h: w,
        };

        self.display.write_region(Color::BlackWhite, fb, frame);
        // Update red buffer so that it's up to date for future partial refreshes.
        self.display.refresh_partial();
        //self.display.write_region(Color::Red, fb, frame);
    }

    fn button_state(&mut self) -> Buttons {
        self.buttons.button_state()
    }

    fn now_ms(&self) -> u32 {
        esp_hal::time::Instant::now().duration_since_epoch().as_millis() as u32
    }

    fn sleep_ms(&mut self, ms: u32) {
        sleep_ms(ms);
    }

    fn log(&mut self, msg: &str) {
        println!("{}", msg);
    }

    fn low_power_enable(&mut self) {
        self.display.sleep();
    }

    fn low_power_disable(&mut self) {
        // No commands needed.
    }

    fn power_off(&mut self) {
        todo!()
    }
}
