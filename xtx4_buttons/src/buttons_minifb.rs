// Button reader via minifb window key state.

use std::cell::RefCell;
use std::rc::Rc;
use minifb::{Key, Window};
use xtx4_platform_interface::Buttons;
use crate::ButtonReader;

pub struct MinifbButtons {
    window: Rc<RefCell<Window>>,
}

impl MinifbButtons {
    pub fn new(window: Rc<RefCell<Window>>) -> Self {
        Self { window }
    }
}

impl ButtonReader for MinifbButtons {
    fn button_state(&mut self) -> Buttons {
        let mut w = self.window.borrow_mut();
        w.update();

        let mut state = Buttons::empty();
        if w.is_key_down(Key::D) { state |= Buttons::LEFT_OUTER; }
        if w.is_key_down(Key::F) { state |= Buttons::LEFT_INNER; }
        if w.is_key_down(Key::J) { state |= Buttons::RIGHT_INNER; }
        if w.is_key_down(Key::K) { state |= Buttons::RIGHT_OUTER; }
        if w.is_key_down(Key::L) { state |= Buttons::SIDE_TOP; }
        if w.is_key_down(Key::Semicolon) { state |= Buttons::SIDE_BOTTOM; }
        if w.is_key_down(Key::P) { state |= Buttons::POWER; }
        state
    }
}
