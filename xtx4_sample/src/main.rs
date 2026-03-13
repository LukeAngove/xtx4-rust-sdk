#![no_std]
use xtx4_platform::{Platform, Button, InputState, Canvas, init};
use embedded_graphics::{
    prelude::*,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    text::Text,
};

fn main() {
    let mut platform = init();
    app_main(&mut platform);
}

fn app_main(platform: &mut impl Platform) {
    let mut input = InputState::new();

    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);

    let mut fb = [0u8; 800 * 480 / 8];
    fb.fill(0xFF); // start with white screen
    platform.display_flush(&fb);

    let mut buf = [0xffu8; (480 * 800 + 7) / 8];
    let mut canvas = Canvas::new(&mut buf, 480, 800);
    Text::new("Hello X4!", Point::new(10, 20), style).draw(&mut canvas).expect("Invalid draw!");
    platform.display_flush(&buf);

    let mut buf = [0xffu8; (100 * 100 + 7) / 8];
    let mut canvas = Canvas::new(&mut buf, 100, 100);
    Text::new("Hi!", Point::new(0, 10), style).draw(&mut canvas).expect("Invalid draw!");
    platform.display_flush_partial(&buf, 50, 50, 100, 100);

    loop {
        input.update(platform);

        if input.was_pressed(Button::LeftOuter) {
            // Full refresh - draw black rectangle top left
            draw_rect(&mut fb, 0, 0, 100, 100, false);
            platform.display_flush(&fb);
        }

        if input.was_pressed(Button::LeftInner) {
            // Full refresh - clear to white
            fb.fill(0xFF);
            platform.display_flush(&fb);
        }

        if input.was_pressed(Button::RightInner) {
            // Partial refresh - draw small black square
            let small_fb = [0u8; 100 * 100 / 8];
            platform.display_flush_partial(&small_fb, 200, 200, 100, 100);
        }

        if input.was_pressed(Button::RightOuter) {
            // Partial refresh - clear same region
            let small_fb = [0xFF; 100 * 100 / 8];
            platform.display_flush_partial(&small_fb, 250, 250, 100, 100);
        }

        if input.was_pressed(Button::SideTop) {
            // Accumulate ghosting with rapid partial refreshes
            for i in 0..5u16 {
                let x = 100 + i * 60;
                let black = [0u8; (50 * 50 + 7) / 8];
                let white = [0xFF; (50 * 50 + 7) / 8];
                platform.display_flush_partial(&black, x, 400, 50, 50);
                platform.display_flush_partial(&white, x, 400, 50, 50);
            }
        }

        if input.was_pressed(Button::SideBottom) {
            // Overlapping partial refreshes at slightly varying positions
            // to demonstrate ghost accumulation
            for i in 0..5u16 {
                let x = 100 + i * 30; // smaller step so regions overlap
                let black = [0u8; (50 * 50 + 7) / 8];
                let white = [0xFFu8; (50 * 50 + 7) / 8];
                platform.display_flush_partial(&black, x, 400, 50, 50);
                platform.display_flush_partial(&white, x, 400, 50, 50);
            }
        }

        if input.is_pressed(Button::Power) {
            platform.power_off();
        }

        platform.sleep_ms(16);
    }
}

/// Draw a filled rectangle into the framebuffer.
/// white=true draws white, white=false draws black.
fn draw_rect(fb: &mut [u8], x: u16, y: u16, w: u16, h: u16, white: bool) {
    for row in 0..h {
        for col in 0..w {
            let px = (x + row) as usize * 480 + (y + col) as usize;
            let byte = px / 8;
            let bit = px % 8;
            if white {
                fb[byte] |= 0x80 >> bit;
            } else {
                fb[byte] &= !(0x80 >> bit);
            }
        }
    }
}
