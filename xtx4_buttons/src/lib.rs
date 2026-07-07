#![cfg_attr(not(target_arch = "x86_64"), no_std)]

use xtx4_platform_interface::Buttons;

pub use xtx4_platform_interface::Buttons as ButtonFlags;

/// Read button state from platform-specific hardware.
pub trait ButtonReader {
    fn button_state(&mut self) -> Buttons;
}

#[cfg(target_arch = "riscv32")]
#[path = "buttons_adc.rs"]
mod buttons_impl;
#[cfg(target_arch = "riscv32")]
pub use buttons_impl::ButtonsAdc;

#[cfg(target_arch = "x86_64")]
#[path = "buttons_stdin.rs"]
mod buttons_impl;
#[cfg(target_arch = "x86_64")]
pub use buttons_impl::ButtonsStdin;

#[cfg(target_arch = "x86_64")]
pub mod buttons_mock;