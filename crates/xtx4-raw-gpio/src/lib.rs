#![cfg_attr(target_arch = "riscv32", no_std)]

//! Zero-sized `OutputPin` for GPIO pins not exposed by esp-hal.
//!
//! Some ESP32-C3 pins are reserved by esp-hal but are physically
//! usable on certain hardware configurations. For example, GPIO12
//! is normally reserved for flash SPIHD, but the Xteink X4 uses DIO
//! flash mode, freeing the pin for SD card chip select. esp-hal
//! won't hand out a typed pin in this case.
//!
//! This crate provides `RawGpioPin<const PIN>` — a zero-sized
//! type that writes GPIO registers directly through the PAC. It
//! implements `embedded-hal::digital::OutputPin`, so it can be used
//! anywhere an `OutputPin` is expected (e.g. as a CS pin for
//! `CriticalSectionDevice`).
//!
//! Only `set_high` and `set_low` are supported. No direction
//! read-back, no input mode, no interrupt support.
//!
//! # Safety
//!
//! You must ensure the pin is physically available and not used
//! for other purposes (e.g. flash memory) before using this type.
//! The crate assumes you know your hardware.

#[cfg(target_arch = "riscv32")]
mod raw {
    use core::convert::Infallible;
    use embedded_hal::digital::{ErrorType, OutputPin};

    /// A zero-sized type representing a raw GPIO pin number.
    ///
    /// Implements `OutputPin` by writing GPIO registers directly.
    /// Only `set_high` and `set_low` are supported — no direction
    /// read-back, no input mode.
    pub struct RawGpioPin<const PIN: u32>;

    impl<const PIN: u32> RawGpioPin<PIN> {
        /// Enable the pin as a GPIO output.
        ///
        /// Writes the GPIO enable register via the PAC. Must be
        /// called before using the pin for CS toggling.
        pub fn new() -> Self {
            // SAFETY: direct register write — no lock needed for GPIO
            let gpio = esp_hal::peripherals::GPIO::regs();
            gpio.enable_w1ts().write(|w| unsafe { w.bits(1 << PIN) });
            Self
        }
    }

    impl<const PIN: u32> ErrorType for RawGpioPin<PIN> {
        type Error = Infallible;
    }

    impl<const PIN: u32> OutputPin for RawGpioPin<PIN> {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            let gpio = esp_hal::peripherals::GPIO::regs();
            gpio.out_w1tc().write(|w| unsafe { w.bits(1 << PIN) });
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            let gpio = esp_hal::peripherals::GPIO::regs();
            gpio.out_w1ts().write(|w| unsafe { w.bits(1 << PIN) });
            Ok(())
        }
    }
}

#[cfg(target_arch = "riscv32")]
pub use raw::RawGpioPin;
