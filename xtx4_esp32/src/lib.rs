#![cfg_attr(not(target_arch = "x86_64"), no_std)]

// This file is a direct port of https://github.com/open-x4-epaper/community-sdk/blob/9f76376a5cc7894cff9ca87bbdd34dab715d8a59/libs/display/EInkDisplay/src/EInkDisplay.cpp
//
// Single crate for both real ESP32 hardware and emulated mock.
// On x86_64 targets the in-memory mock backend is used; on ESP32 targets
// (xtensa, riscv32) the real SPI/GPIO hardware backend is used.

// Hardware pin reference (from community SDK):
//   SPI bus : SCLK=8, MOSI=10
//   EPD CS  : GPIO 21
//   EPD DC  : GPIO 4   (command/data select)
//   EPD RST : GPIO 5
//   EPD BUSY: GPIO 6   (active-high = busy)
//   SD CS   : GPIO 12  (shares SPI bus)
//   Buttons : resistor ladder on ADC (check SDK for voltage thresholds)
//   Battery : GPIO 0, voltage divider — ADC read = 0.5 × actual voltage

#[cfg(not(target_arch = "x86_64"))]
use esp_backtrace as _;
#[cfg(not(target_arch = "x86_64"))]
use esp_println::println;

#[cfg(not(target_arch = "x86_64"))]
mod sleep;
#[cfg(not(target_arch = "x86_64"))]
mod buttons;

#[cfg(target_arch = "x86_64")]
mod emulated;

use xtx4_platform_interface::{Buttons, Framebuffer, Buffer, Rectangle, FRAME_WIDTH, FRAME_HEIGHT, DrawTransform};
use xtx4_platform_interface::Platform as PlatformTrait;
use ssd1677::{ButtonReader, DisplayInterface, Display};

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

// ── Shared Platform struct ──────────────────────────────────────────────────

pub struct Xtx4PlatformInner<T: DisplayInterface, B: ButtonReader> {
    display: Display<T>,
    buttons: B,
}

impl<T: DisplayInterface, B: ButtonReader> PlatformTrait for Xtx4PlatformInner<T, B> {
    fn display_flush(&mut self, fb: &Framebuffer) {
        self.display.flush_full(fb);
    }

    fn display_fast(&mut self, fb: &Framebuffer) {
        self.display.fast_full(fb);
    }

    fn display_flush_partial(&mut self, fb: &Buffer, frame: &Rectangle) {
        self.display.flush_partial(fb, frame);
    }

    fn button_state(&mut self) -> Buttons {
        self.buttons.button_state()
    }

    fn now_ms(&self) -> u32 {
        xtx4_host::now_ms()
    }

    fn sleep_ms(&mut self, ms: u32) {
        xtx4_host::delay_ms(ms);
    }

    fn log(&mut self, msg: &str) {
        println!("{}", msg);
    }

    fn low_power_enable(&mut self) {
        self.display.sleep();
    }

    fn low_power_disable(&mut self) {}

    fn power_off(&mut self) {
        self.display.sleep();
    }
}

// ── ESP32 hardware constructor ──────────────────────────────────────────────

#[cfg(not(target_arch = "x86_64"))]
impl Xtx4PlatformInner<ssd1677::esp_interface::EspInterface, buttons::Xtx4Buttons> {
    pub fn new() -> Self {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        let transport = ssd1677::esp_interface::EspInterface::new(ssd1677::esp_interface::EspInterfaceBuilder {
            spi: peripherals.SPI2.into(),
            sck: peripherals.GPIO8.into(),
            miso: peripherals.GPIO7.into(),
            mosi: peripherals.GPIO10.into(),
            cs: peripherals.GPIO21.into(),
            dc: peripherals.GPIO4.into(),
            rst: peripherals.GPIO5.into(),
            busy: peripherals.GPIO6.into(),
        });
        let display = Display::new(transport, DISPLAY_WIDTH, DISPLAY_HEIGHT);
        let buttons = buttons::Xtx4Buttons::new(
            peripherals.ADC1, peripherals.GPIO1, peripherals.GPIO2, peripherals.GPIO3.into()
        );

        Self { display, buttons }
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub type Xtx4Platform = Xtx4PlatformInner<ssd1677::esp_interface::EspInterface, buttons::Xtx4Buttons>;

// ── Emulated (x86_64) constructor ────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
impl Xtx4PlatformInner<ssd1677::pbm_interface::PbmInterface, emulated::EmulatedButtons> {
    pub fn new() -> Self {
        use emulated::EmulatedButtons;

        let hw = ssd1677::pbm_interface::PbmHardware::new();
        let transport = ssd1677::pbm_interface::PbmInterface::new(hw);
        let display = Display::new(transport, DISPLAY_WIDTH, DISPLAY_HEIGHT);
        let buttons = EmulatedButtons::new();

        Self { display, buttons }
    }
}

#[cfg(target_arch = "x86_64")]
pub type Xtx4Platform = Xtx4PlatformInner<ssd1677::pbm_interface::PbmInterface, emulated::EmulatedButtons>;

