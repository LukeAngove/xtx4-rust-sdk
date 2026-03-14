use crate::canvas::Canvas;
use crate::input::{InputState, InputStateManager};
use embedded_graphics::prelude::*;
use xtx4_platform_interface::{Platform, Framebuffer, FRAME_BYTE_SIZE, FRAME_WIDTH, FRAME_HEIGHT};

#[cfg(feature = "desktop")]
use xtx4_desktop::DesktopPlatform;

#[cfg(feature = "esp32")]
use xtx4_esp32::Esp32Platform;

pub struct XtX4 {
    #[cfg(feature = "desktop")]
    platform: DesktopPlatform,

    #[cfg(feature = "esp32")]
    platform: Esp32Platform,

    input_state_manager: InputStateManager,
    framebuffer: Framebuffer,
}

impl XtX4 {
    pub fn new() -> Self {
        #[cfg(feature = "desktop")]
        let mut platform = DesktopPlatform::new();

        #[cfg(feature = "esp32")]
        let mut platform = Esp32Platform::new();

        let mut input_state_manager = InputStateManager::new();
        _ = input_state_manager.update(&mut platform);
        let framebuffer = [0u8; FRAME_BYTE_SIZE];
        Self{platform, framebuffer, input_state_manager}
    }

    pub fn update_input(&mut self) -> InputState {
        self.input_state_manager.update(&mut self.platform)
    }
     
    pub fn canvas<'a>(&'a mut self) -> Canvas<'a> {
        let canvas = Canvas::new(&mut self.framebuffer, Size::new(FRAME_WIDTH as u32, FRAME_HEIGHT as u32));
        canvas
    }

    /// Push a full framebuffer to the display (or emulated window).
    pub fn display_flush(&mut self) {
        self.platform.display_flush(&self.framebuffer);
    }

    /// Push a full framebuffer to the display (or emulated window).
    pub fn display_full_flush(&mut self, canvas: &Canvas) {
        // TODO Should panic if canvas isn't same size as screen.
        self.platform.display_flush(canvas.buf().try_into().unwrap());
    }

    /// Push a paritial framebuffer to the display (or emulated window).
    pub fn display_partial_at(&mut self, canvas: &Canvas, top_left: Point) {
        let size = canvas.size();
        self.platform.display_flush_partial(canvas.buf(), top_left.x as u16, top_left.y as u16, size.width as u16, size.height as u16);
    }

    /// Push a paritial framebuffer to the display (or emulated window).
    pub fn display_partial_clone(&mut self, canvas: &Canvas) {
        let start = canvas.start();
        let size = canvas.size();
        self.platform.display_flush_partial(canvas.buf(), start.x as u16, start.y as u16, size.width as u16, size.height as u16);
    }

    /// Get the current time in milliseconds
    pub fn now_ms(&self) -> u32 {
        self.platform.now_ms()
    }

    /// Sleep for ms milliseconds, keeping the platform responsive.
    pub fn sleep_ms(&mut self, ms: u32) {
        self.platform.sleep_ms(ms);
    }

    /// Turn off (or close) the device (or app)
    pub fn power_off(&mut self) {
        self.platform.power_off();
    }

    pub fn log(&mut self, msg: &str) {
        self.platform.log(msg);
    }
}

