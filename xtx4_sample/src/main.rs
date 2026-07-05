#![no_std]
#![no_main]
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    prelude::*,
    text::Text,
};
use xtx4_platform::{bit_buf, Canvas, XtX4, STYLE_BLACK};

#[no_mangle]
fn main() {
    let mut platform = XtX4::new();
    // Full clear to white
    {
        let full_canvas = platform.canvas();
        full_canvas.fill(0xFF);
        platform.display_flush();
    }

    // SideTop ghost accumulation test (same as C++ SDK test)
    for i in 0..5i32 {
        let x = 80 + i * 80;

        // Black square
        let mut black = bit_buf!(0u8; (40, 40));
        let black = Canvas::new(&mut black, Size::new(40, 40));
        platform.display_partial_at(&black, Point::new(x, 400));

        // White square (erase)
        let mut white = bit_buf!(0xffu8; (40, 40));
        let white = Canvas::new(&mut white, Size::new(40, 40));
        platform.display_partial_at(&white, Point::new(x, 400));
    }

    platform.log("Done!");

    // Halt - no more logging
    loop { core::hint::spin_loop(); }
}