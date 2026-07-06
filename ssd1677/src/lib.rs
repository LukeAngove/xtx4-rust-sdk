#![cfg_attr(not(target_arch = "x86_64"), no_std)]

pub mod display_transport;
pub mod ssd1677;
pub mod display;
#[cfg(target_arch = "x86_64")]
pub mod mock_transport;

pub use display::Display;
pub use display_transport::{DisplayTransport, ButtonReader};
pub use ssd1677::*;