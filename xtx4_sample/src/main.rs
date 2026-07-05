#![no_std]
#![no_main]
use xtx4_platform::{bit_buf, Button, Canvas, XtX4};
use embedded_graphics::{prelude::*, geometry::Size};

#[no_mangle]
fn main() {
    let mut platform = XtX4::new();

    // Start with white screen
    {
        let full_canvas = platform.canvas();
        full_canvas.fill(0x00);
    }
    platform.display_flush();

    let mut auto_mode = false;
    let mut step_counter = 0;
    let mut step = 0;

    loop {
        let input = platform.update_input();

        // LeftInner (f): reset to white
        if input.was_pressed(Button::LeftInner) {
            step = 0;
            auto_mode = false;
            let c = platform.canvas();
            c.fill(0x00);
            platform.display_flush();
        }

        // RightInner (j): manual advance one step
        if input.was_pressed(Button::RightInner) {
            let app_y = 380 + step * 20;
            if app_y + 20 <= 480 {
                let mut black = bit_buf!(0xffu8; (50, 20));
                let black = Canvas::new(&mut black, Size::new(50, 20));
                platform.display_partial_at(&black, Point::new(350, app_y));
                step += 1;
            }
        }

        // RightOuter (k): toggle auto mode
        if input.was_pressed(Button::RightOuter) {
            auto_mode = !auto_mode;
            step_counter = 0;
            if !auto_mode {
                step = 0;
                let c = platform.canvas();
                c.fill(0x00);
                platform.display_flush();
            }
        }

        if auto_mode {
            step_counter += 1;
            if step_counter >= 50 {
                step_counter = 0;
                let app_y = 380 + step * 20;
                if app_y + 20 <= 480 {
                    let mut black = bit_buf!(0xffu8; (50, 20));
                    let black = Canvas::new(&mut black, Size::new(50, 20));
                    platform.display_partial_at(&black, Point::new(350, app_y));
                    step += 1;
                } else {
                    auto_mode = false;
                }
            }
        }

        if input.was_any_pressed() {
            platform.low_power_activate();
        }

        platform.sleep_ms(10);
    }
}
