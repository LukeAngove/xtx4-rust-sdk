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

    #[cfg(feature = "esp32")]
    platform: xtx4_esp32::Xtx4Platform,

    #[cfg(feature = "mock")]
    platform: xtx4_esp32::Xtx4PlatformInner<
        ssd1677::Ssd1677Controller<ssd1677_pbm::PbmInterface>,
        xtx4_buttons_mock::MockButtons,
    >,

    #[cfg(feature = "minifb")]
    platform: xtx4_esp32::Xtx4PlatformInner<
        ssd1677::Ssd1677Controller<ssd1677_minifb::MinifbInterface>,
        xtx4_buttons_minifb::MinifbButtons,
    >,

    input_state_manager: InputStateManager,
    framebuffer: Framebuffer,
}

impl XtX4 {
    pub fn new() -> Self {
        #[cfg(feature = "desktop")]
        let mut platform = DesktopPlatform::new();

        #[cfg(feature = "esp32")]
        let mut platform = xtx4_esp32::Xtx4Platform::new();

        #[cfg(feature = "mock")]
        let mut platform = xtx4_esp32::Xtx4PlatformInner::new_mock();

        #[cfg(feature = "minifb")]
        let mut platform = {
            use xtx4_platform_interface::{FRAME_WIDTH, FRAME_HEIGHT};
            let display_w = FRAME_HEIGHT as u16;
            let display_h = FRAME_WIDTH as u16;
            let window = std::rc::Rc::new(std::cell::RefCell::new(
                minifb::Window::new("Xteink X4", 480, 800, minifb::WindowOptions::default()).unwrap()
            ));
            let hw = ssd1677_minifb::MinifbHardware::new();
            let interface = ssd1677_minifb::MinifbInterface::new(hw, window.clone());
            let controller = ssd1677::Ssd1677Controller::new(interface, display_w, display_h);
            let display = xtx4_display::Display::new(controller);
            let buttons = xtx4_buttons_minifb::MinifbButtons::new(window);
            let host = xtx4_host::Host::new();
            xtx4_esp32::Xtx4PlatformInner::new_with(display, buttons, host)
        };

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

    pub fn low_power_enable(&mut self) {
        self.platform.low_power_enable();
    }

    pub fn low_power_disable(&mut self) {
        self.platform.low_power_disable();
    }

    pub fn display_sleep(&mut self) {
        self.platform.display_sleep();
    }

    pub fn display_wake(&mut self) {
        self.platform.display_wake();
    }

    pub fn light_sleep(&mut self) {
        self.platform.light_sleep();
    }

    pub fn power_off(&mut self) {
        self.platform.power_off();
    }

    pub fn log(&mut self, msg: &str) {
        self.platform.log(msg);
    }
}