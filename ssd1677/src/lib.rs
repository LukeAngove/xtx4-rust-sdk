#![cfg_attr(not(target_arch = "x86_64"), no_std)]

pub mod controller;
pub mod display_interface;
pub mod ssd1677;
#[cfg(target_arch = "x86_64")]
pub mod pbm_interface;
#[cfg(target_arch = "riscv32")]
pub mod esp_interface;

pub use controller::Ssd1677Controller;
pub use display_interface::DisplayInterface;
pub use ssd1677::*;
