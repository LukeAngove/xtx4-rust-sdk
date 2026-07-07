#![cfg_attr(not(target_arch = "x86_64"), no_std)]

pub mod display_interface;
pub mod ssd1677;
pub mod display;
#[cfg(target_arch = "x86_64")]
pub mod pbm_interface;
#[cfg(target_arch = "riscv32")]
pub mod esp_interface;

pub use display::Display;
pub use display_interface::{DisplayInterface, ButtonReader};
pub use ssd1677::*;
