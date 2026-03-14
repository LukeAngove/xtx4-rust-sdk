#![no_std]

pub const FRAME_HEIGHT : usize = 800;
pub const FRAME_WIDTH : usize = 480;
pub const FRAME_BYTE_SIZE : usize = FRAME_WIDTH*FRAME_HEIGHT/8;

/// 1 bit per pixel, row-major.
/// Bit 7 of byte 0 = pixel (0,0). 1 = white, 0 = black.
pub type Framebuffer = [u8; FRAME_BYTE_SIZE];

bitflags::bitflags! {
    #[derive(Clone, Copy)]
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

pub trait Platform {
    /// Push a full framebuffer to the display (or emulated window).
    fn display_flush(&mut self, fb: &Framebuffer);

    /// Push a paritial framebuffer to the display (or emulated window).
    fn display_flush_partial(&mut self, fb: &[u8], x: u16, y: u16, w: u16, h: u16);

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


