#![cfg_attr(not(feature = "desktop"), no_std)]

pub use xtx4_platform_interface::{Buttons, Framebuffer, Platform};

#[cfg(all(feature = "desktop", feature = "esp32"))]
compile_error!("Features 'desktop' and 'esp32' are mutually exclusive");

#[cfg(not(any(feature = "desktop", feature = "esp32")))]
compile_error!("One of 'desktop' or 'esp32' must be enabled");

#[cfg(feature = "desktop")]
use xtx4_desktop::DesktopPlatform;

#[cfg(feature = "esp32")]
use xtx4_esp32::Esp32Platform;

pub enum Button {
    Power = 0,
    LeftOuter = 1,
    LeftInner = 2,
    RightInner = 3,
    RightOuter = 4,
    SideTop = 5,
    SideBottom = 6,
    Count = 7,
}

impl From<Button> for Buttons {
    fn from(button: Button) -> Buttons {
        // Painc if we pass in 'Count'
        Buttons::from_bits(1 << (button as u8)).unwrap()
    }
}

pub struct InputState {
    released: Buttons,
    press_start_ms: [u32; Button::Count as usize],
    last_scan_ms: u32,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            released: Buttons::empty(),
            press_start_ms: [0u32; Button::Count as usize],
            last_scan_ms: 0,
        }
    }

    pub fn update(&mut self, platform: &mut impl Platform) {
        let raw = platform.button_state();
        let now_ms = platform.now_ms();

        self.released = Buttons::empty();
        self.last_scan_ms = now_ms;

        for i in 0..(Button::Count as usize) {
            let flag = Buttons::from_bits(1 << i).unwrap();
            let was_pressed = self.press_start_ms[i] != 0;
            let is_pressed = raw.contains(flag);

            match (was_pressed, is_pressed) {
                (false, true) => {
                    self.press_start_ms[i] = if now_ms == 0 { 1 } else { now_ms };
                }
                (true, false) => {
                    self.press_start_ms[i] = 0;
                    self.released |= flag;
                }
                _ => {}
            }
        }
    }

    pub fn is_pressed(&self, btn: Button) -> bool {
        self.press_start_ms[btn as usize] != 0
    }

    pub fn was_pressed(&self, btn: Button) -> bool {
        self.press_start_ms[btn as usize] == self.last_scan_ms
    }

    pub fn was_released(&self, btn: Button) -> bool {
        self.released.contains(btn.into())
    }

    pub fn was_any_pressed(&self) -> bool {
        self.press_start_ms.iter().any(|&t| t == self.last_scan_ms)
    }

    pub fn was_any_released(&self) -> bool {
        !self.released.is_empty()
    }

    pub fn held_ms(&self, btn: Button, now_ms: u32) -> u32 {
        let start = self.press_start_ms[btn as usize];
        if start == 0 { 0 } else { now_ms - start }
    }
}

pub fn init() -> impl Platform {
    #[cfg(feature = "desktop")]
    return DesktopPlatform::new();

    #[cfg(feature = "esp32")]
    return Esp32Platform::new();
}
