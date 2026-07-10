// Desktop emulator backend — enabled with feature = "desktop".
//
// Renders the 1bpp framebuffer into an 800x480 minifb window.
//
// Button mapping:
//   Up     -> Arrow Up
//   Down   -> Arrow Down
//   Left   -> Arrow Left
//   Right  -> Arrow Right
//   Select -> Enter
//

use minifb::{Key, Window, WindowOptions};
use xtx4_platform_interface::{Buttons, Framebuffer, Buffer, Platform, bit_buf, DrawTransform, Rectangle};

// Use width and height as the viewer sees them.
// This is different to the hardware, which is 90 degrees off!
const HEIGHT : usize = 800;
const WIDTH : usize = 480;
const BUFF_SIZE : usize = HEIGHT * WIDTH;

const FLASH_CYCLES: usize = 3;
const FLASH_PHASE_MS: u64 = 80;

const BLACK : u32 = 0x001C1C1A;
const WHITE : u32 = 0x00F0ECD8;

const DARK_GHOST : u8 = 60;
const LIGHT_GHOST : u8 = 190;

/// Unpack a 1bpp landscape framebuffer into the portrait pixel_buf,
fn unpack(pixels: &mut [u32], fb: &Framebuffer, invert: bool) {
    let cells = fb.as_array_of_cells();
    for (i, byte) in cells.iter().enumerate() {
        for bit in 0..8 {
            let px = i * 8 + bit;
            let is_white = (byte.get() & (0x80 >> bit)) != 0;
            let is_white = if invert { !is_white } else { is_white };
            pixels[px] = if is_white { WHITE } else { BLACK };
        }
    }
}

fn u8_to_pixel(v: u8) -> u32 {
    // Interpolate between BLACK and WHITE
    let t = v as f32 / 255.0;
    let r = (((BLACK >> 16) & 0xFF) as f32 * (1.0 - t) + ((WHITE >> 16) & 0xFF) as f32 * t) as u32;
    let g = (((BLACK >> 8) & 0xFF) as f32 * (1.0 - t) + ((WHITE >> 8) & 0xFF) as f32 * t) as u32;
    let b = ((BLACK & 0xFF) as f32 * (1.0 - t) + (WHITE & 0xFF) as f32 * t) as u32;
    (r << 16) | (g << 8) | b
}

fn fb_bit(fb: &Framebuffer, px: usize) -> bool {
    let cells = fb.as_array_of_cells();
    let byte = px / 8;
    let bit = px % 8;
    (cells[byte].get() & (0x80 >> bit)) != 0
}

pub struct DesktopTransform;

impl DrawTransform for DesktopTransform {
    fn stride(full_width: u16, _full_height: u16) -> u16 {
        full_width
    }

    fn apply(x: u16, y: u16, width: u16, height: u16) -> Option<(u16, u16)> {
        if x < width && y < height {
            Some((x,y))
        } else {
            None
        }
    }
}

pub struct DesktopPlatform {
    window: Window,
    prev_buf: Framebuffer,
    ghost_buf: [u8; BUFF_SIZE],
}

impl DesktopPlatform {
    pub fn new() -> Self {
        let window = Window::new(
            "Xteink X4 — Desktop Emulator",
            WIDTH,
            HEIGHT,
            WindowOptions::default(),
        )
        .expect("Failed to open emulator window");

        Self {
            window,
            prev_buf: bit_buf!(0u8; (WIDTH, HEIGHT)),
            ghost_buf: [0; BUFF_SIZE],
        }
    }

    fn render_ghost(&mut self) {
        let mut pixel_buf: Vec<u32> = vec![0; BUFF_SIZE];
        for px in 0..BUFF_SIZE {
            pixel_buf[px] = u8_to_pixel(self.ghost_buf[px]);
        }
        self.window
            .update_with_buffer(&pixel_buf, WIDTH, HEIGHT)
            .unwrap();
    }

    fn apply_ghost(&mut self, fb: &Buffer, x: u16, y: u16, w: u16, h: u16) {
        let cells = fb.as_slice_of_cells();

        for row in 0..h as usize {
            for col in 0..w as usize {
                let src_px = row * w as usize + col;
                let src_byte = src_px / 8;
                let src_bit = src_px % 8;
                let new_white = (cells[src_byte].get() & (0x80 >> src_bit)) != 0;

                let dst_px = (y as usize + row) * WIDTH + (x as usize + col);
                let prev_white = fb_bit(&self.prev_buf, dst_px);

                self.ghost_buf[dst_px] = if prev_white == new_white {
                    let target = if new_white { 255 } else { 0 };
                    let current = self.ghost_buf[dst_px];
                    (current as i16 + (target as i16 - current as i16) / 4) as u8
                } else if prev_white && !new_white {
                    DARK_GHOST
                } else {
                    LIGHT_GHOST
                };
            }
        }
    }

    fn reset_ghost(&mut self,fb: &Framebuffer) {
        for px in 0..BUFF_SIZE {
            self.ghost_buf[px] = if fb_bit(fb, px) { 255 } else { 0 };
        }
    }

    /// Returns false when the window has been closed.
    pub fn is_open(&self) -> bool {
        self.window.is_open()
    }

    fn sleep_and_render(&mut self, ms: u64, pixels: &[u32]) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        self.window
            .update_with_buffer(&pixels, WIDTH, HEIGHT)
            .unwrap();
    }
}

impl Platform for DesktopPlatform {
    fn display_flush(&mut self, fb: &Framebuffer) {
        let mut pixel_buf: Vec<u32> = vec![0; BUFF_SIZE];

        for _ in 0..FLASH_CYCLES {
            // Inverted previous image (ghosting)
            unpack(&mut pixel_buf, &self.prev_buf, true);
            self.sleep_and_render(FLASH_PHASE_MS, &pixel_buf);

            // Pure black
            pixel_buf.fill(BLACK);
            self.sleep_and_render(FLASH_PHASE_MS, &pixel_buf);

            self.reset_ghost(&fb);

            // Inverted new image
            unpack(&mut pixel_buf, &fb, true);
            self.sleep_and_render(FLASH_PHASE_MS, &pixel_buf);

            // Pure white
            pixel_buf.fill(WHITE);
            self.sleep_and_render(FLASH_PHASE_MS, &pixel_buf);
        }

        unpack(&mut pixel_buf, &fb, false);
        self.sleep_and_render(FLASH_PHASE_MS, &pixel_buf);

        self.prev_buf = fb.clone();
    }

    fn display_fast(&mut self, fb: &Framebuffer) {
        let mut pixel_buf: Vec<u32> = vec![0; BUFF_SIZE];

        unpack(&mut pixel_buf, &fb, false);
        self.sleep_and_render(FLASH_PHASE_MS, &pixel_buf);

        self.prev_buf = fb.clone();
    }



    fn display_flush_partial(&mut self, fb: &Buffer, rect: &Rectangle) {
        let Rectangle{x, y, w, h} = *rect;
        self.apply_ghost(fb, x, y, w, h);

        let fb_cells = fb.as_slice_of_cells();

        for row in 0..h as usize {
            for col in 0..w as usize {
                let src_px = row * w as usize + col;
                let src_byte = src_px / 8;
                let src_bit = src_px % 8;
                let new_white = (fb_cells[src_byte].get() & (0x80 >> src_bit)) != 0;

                let dst_px = (y as usize + row) * WIDTH + (x as usize + col);
                let prev_white = fb_bit(&self.prev_buf, dst_px);

                if prev_white != new_white {
                    self.ghost_buf[dst_px] = 255 - self.ghost_buf[dst_px];
                }
            }
        }
        self.render_ghost();
        std::thread::sleep(std::time::Duration::from_millis(50));

        self.apply_ghost(fb, x, y, w, h);
        self.render_ghost();
        std::thread::sleep(std::time::Duration::from_millis(100));

        let cells = self.prev_buf.as_array_of_cells();

        for row in 0..h as usize {
            for col in 0..w as usize {
                let src_px = row * w as usize + col;
                let src_byte = src_px / 8;
                let src_bit = src_px % 8;
                let new_white = (fb_cells[src_byte].get() & (0x80 >> src_bit)) != 0;

                let dst_px = (y as usize + row) * WIDTH + (x as usize + col);
                let dst_byte = dst_px / 8;
                let dst_bit = dst_px % 8;

                if new_white {
                    cells[dst_byte].set(cells[dst_byte].get() | 0x80 >> dst_bit);
                } else {
                    cells[dst_byte].set(cells[dst_byte].get() & !(0x80 >> dst_bit));
                }
            }
        }
    }

    fn button_state(&mut self) -> Buttons {
        // update() must be called regularly for minifb to process events.
        // If you're not calling display_flush every frame, call this instead.
        self.window.update();

        let mut state = Buttons::empty();

        if self.window.is_key_down(Key::D) { state |= Buttons::LEFT_OUTER; }
        if self.window.is_key_down(Key::F) { state |= Buttons::LEFT_INNER; }
        if self.window.is_key_down(Key::J) { state |= Buttons::RIGHT_INNER; }
        if self.window.is_key_down(Key::K) { state |= Buttons::RIGHT_OUTER; }
        if self.window.is_key_down(Key::L) { state |= Buttons::SIDE_TOP; }
        if self.window.is_key_down(Key::Semicolon) { state |= Buttons::SIDE_BOTTOM; }
        if self.window.is_key_down(Key::P) { state |= Buttons::POWER; }

        state
    }

    fn now_ms(&self) -> u32 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32
    }

    fn sleep_ms(&mut self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
        self.window.update(); // keep the window alive during sleeps
    }

    fn power_off(&mut self) {
        std::process::exit(0);
    }

    fn log(&mut self, msg: &str) {
        println!("{}", msg);
    }

    fn low_power_enable(&mut self) {}

    fn low_power_disable(&mut self) {}

    fn display_sleep(&mut self) {}
    fn display_wake(&mut self) {}

    fn light_sleep(&mut self) {}
}
