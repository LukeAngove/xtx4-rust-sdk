#![cfg_attr(not(target_arch = "x86_64"), no_std)]

use xtx4_platform_interface::Buttons;

pub use xtx4_platform_interface::Buttons as ButtonFlags;

/// Read button state from platform-specific hardware.
pub trait ButtonReader {
    fn button_state(&mut self) -> Buttons;
}
