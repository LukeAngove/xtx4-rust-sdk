use crate::input::{InputState, InputStateManager};
use embedded_graphics::prelude::*;
use xtx4_platform_interface::{bit_buf, Buffer, Framebuffer, Platform, Rectangle, FRAME_HEIGHT, FRAME_WIDTH};

#[cfg(feature = "desktop")]
use xtx4_desktop::DesktopPlatform;

#[cfg(feature = "desktop")]
pub type Canvas<'a> = crate::canvas::Canvas<'a, xtx4_desktop::DesktopTransform>;

#[cfg(not(feature = "desktop"))]
pub type Canvas<'a> = crate::canvas::Canvas<'a, xtx4_esp32::Esp32Transform>;

pub struct XtX4 {
    #[cfg(feature = "desktop")]
    platform: DesktopPlatform,

    #[cfg(all(not(feature = "desktop"), target_arch = "x86_64"))]
    platform: xtx4_esp32::Xtx4Platform<xtx4_esp32::mock_transport::MockTransport, xtx4_esp32::emulated::EmulatedButtons>,

    #[cfg(all(not(feature = "desktop"), not(target_arch = "x86_64")))]
    platform: xtx4_esp32::Xtx4Platform<xtx4_esp32::esp_transport::EspTransport, xtx4_esp32::buttons::Xtx4Buttons>,

    input_state_manager: InputStateManager,
    framebuffer: Framebuffer,
}

impl XtX4 {
    pub fn new() -> Self {
        #[cfg(feature = "desktop")]
        let mut platform = DesktopPlatform::new();

        #[cfg(all(not(feature = "desktop"), target_arch = "x86_64"))]
        let mut platform = xtx4_esp32::Xtx4Platform::new();

        #[cfg(all(not(feature = "desktop"), not(target_arch = "x86_64")))]
        let mut platform = xtx4_esp32::Xtx4Platform::new();

        let mut input_state_manager = InputStateManager::new();
        input_state_manager.update(&mut platform);
        let framebuffer = bit_buf!(0u8; (FRAME_WIDTH, FRAME_HEIGHT));
        Self {
            platform,
            framebuffer,
            input_state_manager,
        }
    }

    pub fn update_input(&mut self) -> InputState {
        self.input_state_manager.update(&mut self.platform)
    }

    pub fn canvas<'a>(&'a mut self) -> Canvas<'a> {
        let canvas = Canvas::new(
            &mut self.framebuffer,
            Size::new(FRAME_WIDTH as u32, FRAME_HEIGHT as u32),
        );
        canvas
    }

    pub fn display_flush(&mut self) {
        self.platform.display_flush(&self.framebuffer);
    }

    pub fn display_full_flush(&mut self, canvas: &Canvas) {
        let arr: &Framebuffer = unsafe { &*(canvas.buf() as *const Buffer as *const Framebuffer) };
        self.platform.display_flush(arr);
    }

    pub fn display_fast(&mut self) {
        self.platform.display_fast(&self.framebuffer);
    }

    pub fn display_partial_at(&mut self, canvas: &Canvas, top_left: Point) {
        let size = canvas.size();
        self.platform.display_flush_partial(
            canvas.buf(),
            &Rectangle{
                x: top_left.x as u16,
                y: top_left.y as u16,
                w: size.width as u16,
                h: size.height as u16,
            }
        );
    }

    pub fn display_partial_clone(&mut self, canvas: &Canvas) {
        let start = canvas.start();
        let size = canvas.size();
        self.platform.display_flush_partial(
            canvas.buf(),
            &Rectangle{
                x: start.x as u16,
                y: start.y as u16,
                w: size.width as u16,
                h: size.height as u16,
            }
        );
    }

    pub fn now_ms(&self) -> u32 {
        self.platform.now_ms()
    }

    pub fn sleep_ms(&mut self, ms: u32) {
        self.platform.sleep_ms(ms);
    }

    pub fn low_power_activate(&mut self) {
        self.platform.low_power_enable();
    }

    pub fn power_off(&mut self) {
        self.platform.power_off();
    }

    pub fn log(&mut self, msg: &str) {
        self.platform.log(msg);
    }
}