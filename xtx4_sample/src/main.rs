#![no_std]
#![no_main]
use xtx4_platform::{XtX4, Button, Canvas, STYLE_BLACK, bit_buf};
use embedded_graphics::{
    prelude::*,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    text::Text,
    primitives::{Rectangle, PrimitiveStyleBuilder},
};

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

        full_canvas.fill(0xFF); // start with white screen
        platform.display_flush();
    }

    let buf = bit_buf!(0xffu8; (480, 800));
    let mut canvas = Canvas::new(&buf, Size::new(480, 800));
    Text::new("Hello X4! I'm here!", Point::new(10, 20), text_style).draw(&mut canvas).expect("Invalid draw!");
    platform.display_full_flush(&canvas);

    platform.log("Into loop!");

    //let buf = bit_buf!(0xffu8; (100, 100));
    //let mut canvas = Canvas::new(&buf, Size::new(100,100));
    //Text::new("Hi!", Point::new(0, 10), text_style).draw(&mut canvas).expect("Invalid draw!");
    //platform.display_partial_at(&canvas, Point::new(50,50));

    //platform.log("Starting main loop...");

    loop {
        //let input = platform.update_input();

        //if input.was_pressed(Button::LeftOuter) {
        //    // Full refresh - draw black rectangle top left
        //    let full_canvas = platform.canvas();

        //    let [mut top, bottom] = full_canvas.split_vert(&[1, 3]);
        //    let [mut bl, mut br] = bottom.split_horz(&[1,1]);

        //    let rect = Rectangle::new(Point::new(40,40), Size::new(100,100)).into_styled(line_style);
        //    rect.draw(&mut top);
        //    rect.draw(&mut bl);
        //    rect.draw(&mut br);
        //    platform.display_flush();
        //}

        //if input.was_pressed(Button::LeftInner) {
        //    let full_canvas = platform.canvas();
        //    // Full refresh - clear to white
        //    full_canvas.fill(0xFF);
        //    platform.display_flush();
        //}

        //if input.was_pressed(Button::RightInner) {
        //    // Partial refresh - draw small black square
        //    let mut small_fb = bit_buf!(0u8; (100, 100));
        //    let small_canvas = Canvas::new(&mut small_fb, Size::new(100, 100));
        //    platform.display_partial_at(&small_canvas, Point::new(200, 200));
        //}

        //if input.was_pressed(Button::RightOuter) {
        //    // Partial refresh - clear same region
        //    let mut small_fb = bit_buf!(0xffu8; (100, 100));
        //    let small_canvas = Canvas::new(&mut small_fb, Size::new(100, 100));
        //    platform.display_partial_at(&small_canvas, Point::new(250, 250));
        //}

        //if input.was_pressed(Button::SideTop) {
        //    // Accumulate ghosting with rapid partial refreshes
        //    for i in 0..5i32 {
        //        let x = 100 + i * 60;
        //        let mut black = bit_buf!(0u8; (50, 50));
        //        let black = Canvas::new(&mut black, Size::new(50, 50));
        //        let mut white = bit_buf!(0xffu8; (50, 50));
        //        let white = Canvas::new(&mut white, Size::new(50, 50));
        //        platform.display_partial_at(&black, Point::new(x, 400));
        //        platform.display_partial_at(&white, Point::new(x, 400));
        //    }
        //}

        //if input.was_pressed(Button::SideBottom) {
        //    // Overlapping partial refreshes at slightly varying positions
        //    // to demonstrate ghost accumulation
        //    for i in 0..5i32 {
        //        let x = 100 + i * 30; // smaller step so regions overlap
        //        let mut black = bit_buf!(0; (50, 50));
        //        let black = Canvas::new(&mut black, Size::new(50, 50));
        //        let mut white = bit_buf!(0xff; (50, 50));
        //        let white = Canvas::new(&mut white, Size::new(50, 50));
        //        platform.display_partial_at(&black, Point::new(x, 400));
        //        platform.display_partial_at(&white, Point::new(x, 400));
        //    }
        //}

        //if input.is_pressed(Button::Power) {
        //    platform.power_off();
        //}

        platform.log("Loop!");
        platform.sleep_ms(5000);
    }
}
