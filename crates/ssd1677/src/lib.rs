#![cfg_attr(not(target_arch = "x86_64"), no_std)]

pub mod controller;
pub mod display_interface;
pub mod ssd1677;
pub mod lut;

pub use controller::Ssd1677Controller;
pub use display_interface::DisplayInterface;
pub use ssd1677::*;
pub use lut::*;
