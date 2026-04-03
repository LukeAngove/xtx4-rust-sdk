#![no_std]

use core::cell::Cell;

pub const FRAME_HEIGHT: usize = 800;
pub const FRAME_WIDTH: usize = 480;

pub const FRAME_BYTE_SIZE: usize = FRAME_WIDTH * FRAME_HEIGHT / 8;

#[macro_export]
macro_rules! bit_buf {
    ($fill:expr; ($width:expr, $height:expr)) => {
        // Add '7' so we always add an extra byte, unless
        // it lines up exactly to a byte boundary.
        ::core::cell::Cell::new([$fill as u8; ($width * $height + 7) / 8])
    };
}

/// 1 bit per pixel, row-major.
/// Bit 7 of byte 0 = pixel (0,0). 1 = white, 0 = black.
pub type Framebuffer = Cell<[u8; FRAME_BYTE_SIZE]>;
pub type Buffer = Cell<[u8]>;

#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Rectangle {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct Buttons: u8 {
        const POWER       = 1 << 0;
        const LEFT_OUTER  = 1 << 1;
        const LEFT_INNER  = 1 << 2;
        const RIGHT_INNER = 1 << 3;
        const RIGHT_OUTER = 1 << 4;
        const SIDE_TOP    = 1 << 5;
        const SIDE_BOTTOM = 1 << 6;
    }
}

/// Transforms from user buffer space to target
/// buffers space. Mostly used for rotation.
pub trait DrawTransform {
    fn stride(full_width: u16, full_height: u16) -> u16;
    fn apply(x: u16, y: u16, width: u16, height: u16) -> Option<(u16, u16)>;
}

pub trait Platform {
    /// Push a full framebuffer to the display (or emulated window).
    /// Framebuffer is moved.
    fn display_flush(&mut self, fb: &Framebuffer);

    /// Push a paritial framebuffer to the display (or emulated window).
    fn display_fast(&mut self, fb: &Framebuffer);

    /// Push a paritial framebuffer to the display (or emulated window).
    fn display_flush_partial(&mut self, fb: &Buffer, frame: &Rectangle);

    /// Read instantaneous button state.
    fn button_state(&mut self) -> Buttons;

    /// Get the current time in milliseconds
    fn now_ms(&self) -> u32;

    /// Sleep for ms milliseconds, keeping the platform responsive.
    fn sleep_ms(&mut self, ms: u32);

    /// Turn off (or close) the device (or app)
    fn power_off(&mut self);

    /// Log to serial console (or stdout)
    fn log(&mut self, msg: &str);
}
