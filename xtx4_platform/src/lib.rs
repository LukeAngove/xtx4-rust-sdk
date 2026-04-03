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

#[cfg(all(feature = "desktop", feature = "esp32"))]
compile_error!("Features 'desktop' and 'esp32' are mutually exclusive");

#[cfg(not(any(feature = "desktop", feature = "esp32")))]
compile_error!("One of 'desktop' or 'esp32' must be enabled");
