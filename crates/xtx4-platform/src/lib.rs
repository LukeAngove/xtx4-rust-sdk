#![cfg_attr(not(any(feature = "desktop", target_arch = "x86_64")), no_std)]

mod canvas;
mod canvas_split;
mod input;
mod platform;
mod rect_split;

pub use crate::canvas::{STYLE_BLACK, STYLE_WHITE};
pub use crate::input::{Button, InputState};
pub use crate::platform::{XtX4, Canvas};
pub use xtx4_platform_interface::{bit_buf, Buffer, Framebuffer};
pub use sd_storage::{File, SeekFrom};