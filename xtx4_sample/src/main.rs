#![no_std]
#![no_main]
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use xtx4_platform::{bit_buf, Button, Canvas, XtX4, STYLE_BLACK};

#[no_mangle]
fn main() {
    let mut platform = XtX4::new();
    platform.log("Started!");

    let text_style = MonoTextStyle::new(&FONT_6X10, STYLE_BLACK);
    let line_style = PrimitiveStyleBuilder::new()
        .stroke_color(STYLE_BLACK)
        .stroke_width(2)
        .fill_color(STYLE_BLACK)
        .build();
    {
        let full_canvas = platform.canvas();

        full_canvas.fill(0x00); // start with white screen (0=white in e-ink)
        platform.display_flush();
    }

    let buf = bit_buf!(0xffu8; (480, 800));
    let mut canvas = Canvas::new(&buf, Size::new(480, 800));
    Text::new("Blue, Purple! asdf!", Point::new(10, 20), text_style)
        .draw(&mut canvas)
        .expect("Invalid draw!");
    Text::new(
            "Luke, Rodger! blue! Still2!",
            Point::new(20, 30),
            text_style,
        )
        .draw(&mut canvas)
        .expect("Invalid draw!");

    platform.display_full_flush(&canvas);

    // Skip small-canvas drawing (crashes with rotated transforms).
    // Instead, auto-run SideTop sequence for testing.
    {
        let full_canvas = platform.canvas();
        full_canvas.fill(0x00); // start with white screen for SideTop test
        platform.display_fast();
    }
    platform.log("Auto SideTop test...");
    for i in 0..5i32 {
        let x = 80 + i * 80;
        let mut black = bit_buf!(0xffu8; (40, 40));
        let black = Canvas::new(&mut black, Size::new(40, 40));
        platform.display_partial_at(&black, Point::new(x, 400));
    }
    platform.log("SideTop test complete. Entering main loop.");

    platform.log("Starting main loop...");

    let mut idle_counter = 0;
    const SLEEP_COUNT: usize = 200;

    loop {
        let input = platform.update_input();

        if input.was_any_pressed() {
            idle_counter = 0;
        } else {
            idle_counter += 1;
        }

        if input.was_pressed(Button::LeftOuter) {
            // Full refresh - draw black rectangle top left
            let full_canvas = platform.canvas();

            let [mut top, bottom] = full_canvas.split_vert(&[1, 3]);
            let [mut bl, mut br] = bottom.split_horz(&[1, 1]);

            let rect =
                Rectangle::new(Point::new(380, 0), Size::new(100, 100)).into_styled(line_style);
            rect.draw(&mut top);
            let rect =
                Rectangle::new(Point::new(0, 0), Size::new(100, 100)).into_styled(line_style);
            rect.draw(&mut bl);
            rect.draw(&mut br);
            platform.display_fast();
        }

        if input.was_pressed(Button::LeftInner) {
            let full_canvas = platform.canvas();
            // Full refresh - clear to white
            full_canvas.fill(0xFF);
            platform.display_flush();
        }

        if input.was_pressed(Button::RightInner) {
            // Partial refresh - draw small black square
            let mut small_fb = bit_buf!(0u8; (80, 80));
            let small_canvas = Canvas::new(&mut small_fb, Size::new(80, 80));
            platform.display_partial_at(&small_canvas, Point::new(200, 200));
        }

        if input.was_pressed(Button::RightOuter) {
            // Partial refresh - clear same region
            let mut small_fb = bit_buf!(0xffu8; (80, 80));
            let small_canvas = Canvas::new(&mut small_fb, Size::new(80, 80));
            platform.display_partial_at(&small_canvas, Point::new(248, 248));
        }

        if input.was_pressed(Button::SideTop) {
            // Accumulate ghosting with rapid partial refreshes
            for i in 0..5i32 {
                let x = 80 + i * 80;
                let mut black = bit_buf!(0u8; (40, 40));
                let black = Canvas::new(&mut black, Size::new(40, 40));
                let mut white = bit_buf!(0xffu8; (40, 40));
                let white = Canvas::new(&mut white, Size::new(40, 40));
                platform.display_partial_at(&black, Point::new(x, 400));
                platform.display_partial_at(&white, Point::new(x, 400));
            }
        }

        if input.was_pressed(Button::SideBottom) {
            // Overlapping partial refreshes at slightly varying positions
            // to demonstrate ghost accumulation
            for i in 0..5i32 {
                let x = 120 + i * 40; // smaller step so regions overlap
                let mut black = bit_buf!(0; (40, 40));
                let black = Canvas::new(&mut black, Size::new(40, 40));
                let mut white = bit_buf!(0xff; (40, 40));
                let white = Canvas::new(&mut white, Size::new(40, 40));
                platform.display_partial_at(&black, Point::new(x, 400));
                platform.display_partial_at(&white, Point::new(x, 400));
            }
        }

        //if input.is_pressed(Button::Power) {
        //    platform.power_off();
        //}

        if idle_counter > SLEEP_COUNT {
            platform.low_power_activate();
        }

        platform.sleep_ms(10);
    }
}
