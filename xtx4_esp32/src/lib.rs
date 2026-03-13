#![no_std]
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

use esp_hal::spi::master::Spi;
use xtx4_platform_interface::{Buttons, Framebuffer, Platform};

pub struct Esp32Platform {
    // Fill these in once you've set up esp-hal peripherals in main():
    //
    // spi:  Spi<'static, Blocking>,
    // cs:   Output<'static>,   // GPIO 21
    // dc:   Output<'static>,   // GPIO 4
    // rst:  Output<'static>,   // GPIO 5
    // busy: Input<'static>,    // GPIO 6
    // adc:  AdcPin<...>,       // buttons
}

impl Esp32Platform {
    pub fn new() -> Self {
        Esp32Platform {  }
    }
}

impl Platform for Esp32Platform {
    fn display_flush(&mut self, _fb: &Framebuffer) {
        // TODO: port EpdScreenController init + refresh sequence from the
        // community SDK C++ source. Rough steps:
        //   1. Pulse RST low then high to wake display
        //   2. Send SSD1677 init command sequence over SPI
        //   3. Write framebuffer bytes via SPI
        //   4. Send refresh command (0x20)
        //   5. Poll BUSY pin until low
        todo!()
    }

    fn display_flush_partial(&mut self, fb: &[u8], x: u16, y: u16, w: u16, h: u16) {
        todo!()
    }

    fn button_state(&mut self) -> Buttons {
        // TODO: read ADC, map voltage ranges to buttons.
        // Thresholds are in the community SDK hardware lib.
        todo!()
    }

    fn now_ms(&self) -> u32 {
        esp_hal::time::now().duration_since_epoch().to_millis() as u32
    }

    fn sleep_ms(&mut self, ms: u32) {
        esp_hal::delay::Delay::new().delay_millis(ms);
    }

    fn power_off(&mut self) {
        todo!()
    }
}
