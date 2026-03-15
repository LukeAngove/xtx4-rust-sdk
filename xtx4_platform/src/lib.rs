#![cfg_attr(not(feature = "desktop"), no_std)]

mod canvas;
mod canvas_split;
mod rect_split;
mod input;
mod platform;

pub use crate::input::{InputState, Button};
pub use crate::platform::XtX4;
pub use crate::canvas::{Canvas, STYLE_BLACK, STYLE_WHITE};
pub use xtx4_platform_interface::{Buffer, Framebuffer, bit_buf};

#[cfg(all(feature = "desktop", feature = "esp32"))]
compile_error!("Features 'desktop' and 'esp32' are mutually exclusive");

#[cfg(not(any(feature = "desktop", feature = "esp32")))]
compile_error!("One of 'desktop' or 'esp32' must be enabled");

