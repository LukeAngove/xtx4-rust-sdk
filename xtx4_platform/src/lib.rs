#![cfg_attr(not(feature = "desktop"), no_std)]

mod canvas;
mod canvas_split;
mod input;
mod platform;
mod rect_split;

pub use crate::canvas::{STYLE_BLACK, STYLE_WHITE};
pub use crate::input::{Button, InputState};
pub use crate::platform::{XtX4, Canvas};
pub use xtx4_platform_interface::{bit_buf, Buffer, Framebuffer};

#[cfg(any(all(feature = "desktop", feature = "esp32"),
           all(feature = "desktop", feature = "emulated"),
           all(feature = "esp32", feature = "emulated")))]
compile_error!("Features 'desktop', 'esp32', and 'emulated' are mutually exclusive");

#[cfg(not(any(feature = "desktop", feature = "esp32", feature = "emulated")))]
compile_error!("One of 'desktop', 'esp32', or 'emulated' must be enabled");
