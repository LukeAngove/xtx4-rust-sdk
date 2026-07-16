#![no_std]
#![no_main]
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use xtx4_platform::{bit_buf, Button, Canvas, File, SeekFrom, XtX4, STYLE_BLACK};

#[no_mangle]
fn main() {
    #[cfg(feature = "mock")]
    {
        use xtx4_buttons_mock::MockButtons;
        use xtx4_buttons::ButtonFlags;
        // Queue the test sequence: all buttons in order.
        // MockButtons::new() already fills one empty read for XtX4::new().
        MockButtons::queue(ButtonFlags::LEFT_OUTER);
        MockButtons::queue(ButtonFlags::LEFT_INNER);
        MockButtons::queue(ButtonFlags::RIGHT_INNER);
        MockButtons::queue(ButtonFlags::RIGHT_OUTER);
        MockButtons::queue(ButtonFlags::RIGHT_INNER); // repeat
        MockButtons::queue(ButtonFlags::SIDE_TOP);
        MockButtons::queue(ButtonFlags::SIDE_BOTTOM);
        // Power-management demos (hold POWER + face button, fire on release).
        // Each combo requires two reads: pressed (POWER|btn) then empty (release).
        MockButtons::queue(ButtonFlags::POWER | ButtonFlags::LEFT_OUTER);
        MockButtons::queue(ButtonFlags::empty());  // release → low_power_enable
        MockButtons::queue(ButtonFlags::POWER | ButtonFlags::LEFT_INNER);
        MockButtons::queue(ButtonFlags::empty());  // release → low_power_disable
        MockButtons::queue(ButtonFlags::POWER | ButtonFlags::RIGHT_INNER);
        MockButtons::queue(ButtonFlags::empty());  // release → light_sleep
        MockButtons::queue(ButtonFlags::POWER | ButtonFlags::RIGHT_OUTER);
        MockButtons::queue(ButtonFlags::empty());  // release → power_off
    }

    let mut platform = XtX4::new();
    platform.log("Started!");

    let text_style = MonoTextStyle::new(&FONT_6X10, STYLE_BLACK);
    let line_style = PrimitiveStyleBuilder::new()
        .stroke_color(STYLE_BLACK)
        .stroke_width(2)
        .fill_color(STYLE_BLACK)
        .build();

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

    let buf = bit_buf!(0xffu8; (80, 80));
    let mut canvas = Canvas::new(&buf, Size::new(80,80));
    Text::new("Hi!", Point::new(0, 10), text_style).draw(&mut canvas).expect("Invalid draw!");
    platform.display_partial_at(&canvas, Point::new(40,40));

    platform.log("Starting main loop...");

    let mut pwr_action: Option<u8> = None;
    let mut counter = 0;

    loop {
        let input = platform.update_input();

        // Record combo during POWER hold.
        if input.is_pressed(Button::Power) {
            if input.was_pressed(Button::LeftOuter)  { pwr_action = Some(1); }
            if input.was_pressed(Button::LeftInner)  { pwr_action = Some(2); }
            if input.was_pressed(Button::RightInner) { pwr_action = Some(3); }
            if input.was_pressed(Button::RightOuter) { pwr_action = Some(4); }
            // CPU calibration: fires immediately
            if input.was_pressed(Button::SideTop) {
                let start = platform.now_ms();
                let mut count: u32 = 0;
                while platform.now_ms() - start < 500 {
                    count = count.wrapping_add(1);
                }
                let chars = b"0123456789ABCDEF";
                let mut buf = [b'0'; 8];
                let mut n = count as usize;
                for i in 0..8 {
                    buf[7 - i] = chars[n & 0xF];
                    n >>= 4;
                }
                let s = unsafe { core::str::from_utf8_unchecked(&buf) };
                platform.log(s);
            }
        }

        // Fire action on POWER release.
        if input.was_released(Button::Power) {
            match pwr_action {
                Some(1) => { platform.low_power_enable(); platform.display_sleep(); }
                Some(2) => { platform.display_wake(); platform.low_power_disable(); }
                Some(3) => { platform.light_sleep(); platform.log("Woke from light sleep"); }
                Some(4) => platform.power_off(),
                _ => {}
            }
            pwr_action = None;
        }

        // Normal handlers — only when POWER is not held.
        if !input.is_pressed(Button::Power) {
            if input.was_pressed(Button::LeftOuter) {
                let full_canvas = platform.canvas();

                let [mut top, bottom] = full_canvas.split_vert(&[1, 3]);
                let [mut bl, mut br] = bottom.split_horz(&[1, 1]);

                let rect =
                    Rectangle::new(Point::new(40, 40), Size::new(100, 100)).into_styled(line_style);
                rect.draw(&mut top);
                rect.draw(&mut bl);
                rect.draw(&mut br);
                platform.display_fast();
            }

            if input.was_pressed(Button::LeftInner) {
                let full_canvas = platform.canvas();
                full_canvas.fill(0xFF);
                platform.display_flush();
            }

            if input.was_pressed(Button::RightInner) {
                // Storage test: write a checkerboard pattern, read it back, display it.
                const SIZE: usize = 80;
                const RAW: usize = (SIZE * SIZE) / 8;

                // Build a checkerboard: alternating black/white columns.
                let mut pattern = [0u8; RAW];
                for row in 0..SIZE {
                    for col in 0..SIZE {
                        let px = row * SIZE + col;
                        let byte = px / 8;
                        let bit = px % 8;
                        let is_black = ((row / 8) + (col / 8)) % 2 == 0;
                        if is_black {
                            pattern[byte] |= 0x80 >> bit;
                        }
                    }
                }

                let mut readback = [0u8; RAW];
                let sd_result = {
                    let storage = platform.storage();
                    let mut write_ok = false;
                    // Write
                    if let Ok(mut f) = storage.create("/TEST.BIN") {
                        write_ok = f.write(&pattern).is_ok();
                    }
                    if !write_ok {
                        return;
                    }
                    // Read back
                    let mut f = match storage.open("/TEST.BIN") {
                        Ok(f) => f,
                        Err(_) => return,
                    };
                    f.read(&mut readback)
                };
                let n = match sd_result {
                    Ok(n) => n,
                    Err(_) => {
                        platform.log("SD: read failed");
                        return;
                    }
                };
                if n != RAW {
                    platform.log("SD: short read");
                    return;
                }
                // Seek test: jump to middle, read a strip
                let seek_log = {
                    let storage = platform.storage();
                    if let Ok(mut f) = storage.open("/TEST.BIN") {
                        f.seek(SeekFrom::Start(40)).unwrap();
                        let _pos = f.stream_position().unwrap();
                        let _len = f.length().unwrap();
                        let mut strip = [0u8; 80];
                        if f.read(&mut strip).is_ok() { "SD: seek ok" } else { "SD: seek fail" }
                    } else {
                        "SD: seek open fail"
                    }
                };
                platform.log(seek_log);

                let buf = bit_buf!(0u8; (SIZE, SIZE));
                for (cell, byte) in buf.as_array_of_cells()[..RAW]
                    .iter()
                    .zip(readback.iter())
                {
                    cell.set(*byte);
                }
                let canvas = Canvas::new(&buf, Size::new(SIZE as u32, SIZE as u32));
                platform.display_partial_at(&canvas, Point::new(200, 200));
            }

            if input.was_pressed(Button::RightOuter) {
                let mut small_fb = bit_buf!(0xffu8; (80, 80));
                let small_canvas = Canvas::new(&mut small_fb, Size::new(80, 80));
                platform.display_partial_at(&small_canvas, Point::new(248, 248));
            }

            if input.was_pressed(Button::SideTop) {
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
                const PX: usize = 4;
                const BAR_H: usize = 8;
                const MARGIN: i32 = 32;
                let mut col = bit_buf!(0u8; (PX, BAR_H));
                let col = Canvas::new(&mut col, Size::new(PX as u32, BAR_H as u32));
                let bar_y = (platform.height() - BAR_H as u16) as i32 - MARGIN;
                let total_w = platform.width() as i32 - MARGIN * 2;
                for x in (0..total_w).step_by(PX) {
                    platform.display_partial_at(&col, Point::new(MARGIN + x, bar_y));
                }
            }
        }

        if counter % 30 == 0 {
            platform.log("Loop!");
            counter = 0;
        }
        counter += 1;
        platform.sleep_ms(10);
    }
}
