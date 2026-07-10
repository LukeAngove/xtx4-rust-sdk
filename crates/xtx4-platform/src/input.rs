pub use xtx4_platform_interface::{Buttons, Platform};

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

#[derive(Clone, Copy)]
pub struct InputState {
    released: Buttons,
    press_start_ms: [u32; Button::Count as usize],
    scan_ms: u32,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            released: Buttons::empty(),
            press_start_ms: [0u32; Button::Count as usize],
            scan_ms: 1, // Never set to '0', as '0' means 'not pressed'.
        }
    }

    pub fn is_pressed(&self, btn: Button) -> bool {
        self.press_start_ms[btn as usize] != 0
    }

    pub fn was_pressed(&self, btn: Button) -> bool {
        self.press_start_ms[btn as usize] == self.scan_ms
    }

    pub fn was_released(&self, btn: Button) -> bool {
        self.released.contains(btn.into())
    }

    pub fn was_any_pressed(&self) -> bool {
        self.press_start_ms.iter().any(|&t| t == self.scan_ms)
    }

    pub fn was_any_released(&self) -> bool {
        !self.released.is_empty()
    }

    pub fn held_ms(&self, btn: Button, now_ms: u32) -> u32 {
        let start = self.press_start_ms[btn as usize];
        if start == 0 {
            0
        } else {
            now_ms - start
        }
    }
}

pub struct InputStateManager {
    input_state: InputState,
}

impl InputStateManager {
    pub fn new() -> Self {
        Self {
            input_state: InputState::new(),
        }
    }

    pub fn update(&mut self, platform: &mut impl Platform) -> InputState {
        let raw = platform.button_state();
        let now_ms = platform.now_ms();
        // 0 is a special value meaning, 'not pressed', so if we hit
        // exactly 0, shift by 1ms.
        let now_ms = if now_ms == 0 { 1 } else { now_ms };

        let mut next = self.input_state;
        next.released = Buttons::empty();
        next.scan_ms = now_ms;

        for i in 0..(Button::Count as usize) {
            let flag = Buttons::from_bits(1 << i).unwrap();
            let was_pressed = next.press_start_ms[i] != 0;
            let is_pressed = raw.contains(flag);

            match (was_pressed, is_pressed) {
                (false, true) => {
                    next.press_start_ms[i] = now_ms;
                }
                (true, false) => {
                    next.press_start_ms[i] = 0;
                    next.released |= flag;
                }
                _ => {}
            }
        }

        self.input_state = next;
        next
    }
}
