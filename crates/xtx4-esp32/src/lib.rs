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

use xtx4_platform_interface::{Buttons, Framebuffer, Buffer, Rectangle, FRAME_WIDTH, FRAME_HEIGHT, DrawTransform};
use xtx4_platform_interface::Platform as PlatformTrait;
use ssd1677::Ssd1677Controller;
use xtx4_buttons::ButtonReader;
#[cfg(not(target_arch = "x86_64"))]
use ssd1677_esp as ssd1677_esp_impl;
#[cfg(target_arch = "x86_64")]
use ssd1677_pbm as ssd1677_pbm_impl;
use xtx4_display::Display;

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

pub struct Xtx4PlatformInner<D: xtx4_display::DisplayController, B: ButtonReader> {
    display: Display<D>,
    buttons: B,
    host: xtx4_host::Host,
}

impl<D: xtx4_display::DisplayController, B: ButtonReader> Xtx4PlatformInner<D, B> {
    pub fn new_with(display: Display<D>, buttons: B, host: xtx4_host::Host) -> Self {
        Self { display, buttons, host }
    }
}

impl<D: xtx4_display::DisplayController, B: ButtonReader> PlatformTrait for Xtx4PlatformInner<D, B> {
    fn display_flush(&mut self, fb: &Framebuffer) {
        let full = Rectangle { x: 0, y: 0, w: FRAME_WIDTH as u16, h: FRAME_HEIGHT as u16 };
        self.display.flush_full(fb, &full);
    }

    fn display_fast(&mut self, fb: &Framebuffer) {
        let full = Rectangle { x: 0, y: 0, w: FRAME_WIDTH as u16, h: FRAME_HEIGHT as u16 };
        self.display.fast_full(fb, &full);
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

    fn width(&self) -> u16 {
        FRAME_WIDTH as u16
    }

    fn height(&self) -> u16 {
        FRAME_HEIGHT as u16
    }

    fn low_power_enable(&mut self) {
        self.host.set_low_power(true);
    }

    fn low_power_disable(&mut self) {
        self.host.set_low_power(false);
    }

    fn display_sleep(&mut self) {
        self.display.sleep();
    }

    fn display_wake(&mut self) {
        self.display.wake(false);
    }

    fn light_sleep(&mut self) {
        self.display.sleep();
        self.host.light_sleep();
        self.display.wake(false);
    }

    fn power_off(&mut self) {
        self.display.sleep();
        self.host.deep_sleep();
    }
}

// ── ESP32 hardware constructor ──────────────────────────────────────────────

#[cfg(not(target_arch = "x86_64"))]
impl Xtx4PlatformInner<Ssd1677Controller<ssd1677_esp_impl::EspInterface>, xtx4_buttons_adc::ButtonsAdc> {
    pub fn new() -> Self {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        let transport = ssd1677_esp_impl::EspInterface::new(ssd1677_esp_impl::EspInterfaceBuilder {
            spi: peripherals.SPI2.into(),
            sck: peripherals.GPIO8.into(),
            miso: peripherals.GPIO7.into(),
            mosi: peripherals.GPIO10.into(),
            cs: peripherals.GPIO21.into(),
            dc: peripherals.GPIO4.into(),
            rst: peripherals.GPIO5.into(),
            busy: peripherals.GPIO6.into(),
        });
        let controller = Ssd1677Controller::new(transport, DISPLAY_WIDTH, DISPLAY_HEIGHT);
        let display = Display::new(controller);
        // Configure power-pin Input for ButtonsAdc.
        use esp_hal::gpio::{Input, InputConfig, Pull};
        let power = Input::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up));
        let buttons = xtx4_buttons_adc::ButtonsAdc::new(
            peripherals.ADC1, peripherals.GPIO1, peripherals.GPIO2, power
        );
        let host = xtx4_host::Host::new(peripherals.LPWR, 3);

        Self::new_with(display, buttons, host)
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub type Xtx4Platform = Xtx4PlatformInner<Ssd1677Controller<ssd1677_esp_impl::EspInterface>, xtx4_buttons_adc::ButtonsAdc>;

// ── Emulated (x86_64) constructor ────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
impl Xtx4PlatformInner<Ssd1677Controller<ssd1677_pbm_impl::PbmInterface>, xtx4_buttons_stdin::ButtonsStdin> {
    pub fn new() -> Self {

        let hw = ssd1677_pbm_impl::PbmHardware::new();
        let transport = ssd1677_pbm_impl::PbmInterface::new(hw);
        let controller = Ssd1677Controller::new(transport, DISPLAY_WIDTH, DISPLAY_HEIGHT);
        let display = Display::new(controller);
        let buttons = xtx4_buttons_stdin::ButtonsStdin::new();
        let host = xtx4_host::Host::new();

        Self::new_with(display, buttons, host)
    }
}

#[cfg(target_arch = "x86_64")]
pub type Xtx4Platform = Xtx4PlatformInner<Ssd1677Controller<ssd1677_pbm_impl::PbmInterface>, xtx4_buttons_stdin::ButtonsStdin>;

#[cfg(target_arch = "x86_64")]
impl Xtx4PlatformInner<Ssd1677Controller<ssd1677_pbm_impl::PbmInterface>, xtx4_buttons_mock::MockButtons> {
    pub fn new_mock() -> Self {
        let hw = ssd1677_pbm_impl::PbmHardware::new();
        let interface = ssd1677_pbm_impl::PbmInterface::new(hw);
        let controller = Ssd1677Controller::new(interface, DISPLAY_WIDTH, DISPLAY_HEIGHT);
        let display = Display::new(controller);
        let host = xtx4_host::Host::new();
        Self::new_with(display, xtx4_buttons_mock::MockButtons::new(), host)
    }
}

