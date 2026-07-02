// Emulated e-paper display backend — enabled with feature = "emulated".
//
// Uses an in-memory pair of (BW, RED) RAM banks to simulate the SSD1677,
// runs the exact same display pipeline as the ESP32 driver, and renders
// to a minifb window AND diagnostic text to stdout. This lets us reproduce
// and debug differential-update bugs without hardware.

use core::cell::Cell;
use minifb::{Key, Window, WindowOptions};
use xtx4_platform_interface::{
    Buffer, Buttons, DrawTransform, Framebuffer, Platform, Rectangle,
    FRAME_HEIGHT, FRAME_WIDTH,
};

// Display dimensions in landscape (same as hardware)
const DISPLAY_WIDTH: usize = FRAME_HEIGHT; // 800
const DISPLAY_HEIGHT: usize = FRAME_WIDTH; // 480
const DISPLAY_BYTES: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT / 8;

// ── Coordinate transform (same as Esp32Transform) ──────────────────────────

pub struct EmulatedTransform;

impl DrawTransform for EmulatedTransform {
    fn stride(_full_width: u16, full_height: u16) -> u16 {
        full_height
    }

    fn apply(x: u16, y: u16, _width: u16, _height: u16) -> Option<(u16, u16)> {
        let (p_x, p_y) = (y, FRAME_WIDTH as u16 - 1 - x);
        if p_x < FRAME_HEIGHT as u16 && p_y < FRAME_WIDTH as u16 {
            Some((p_x, p_y))
        } else {
            None
        }
    }
}

// ── Data-entry direction ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    Increase,
    Decrease,
}

// ── In-memory SSD1677 mock ─────────────────────────────────────────────────

pub struct MockSsd1677 {
    bw_ram: Cell<[u8; DISPLAY_BYTES]>,
    red_ram: Cell<[u8; DISPLAY_BYTES]>,
    x_dir: Dir,
    y_dir: Dir,
    x_start: u16,
    x_end: u16,
    y_start: u16,
    y_end: u16,
    x_counter: u16,
    y_counter: u16,
}

impl MockSsd1677 {
    pub fn new() -> Self {
        Self {
            bw_ram: Cell::new([0xFF; DISPLAY_BYTES]),
            red_ram: Cell::new([0xFF; DISPLAY_BYTES]),
            x_dir: Dir::Increase,
            y_dir: Dir::Increase,
            x_start: 0,
            x_end: DISPLAY_WIDTH as u16 - 1,
            y_start: 0,
            y_end: DISPLAY_HEIGHT as u16 - 1,
            x_counter: 0,
            y_counter: 0,
        }
    }

    pub fn soft_reset(&mut self) {
        self.x_dir = Dir::Increase;
        self.y_dir = Dir::Increase;
        self.x_start = 0;
        self.x_end = DISPLAY_WIDTH as u16 - 1;
        self.y_start = 0;
        self.y_end = DISPLAY_HEIGHT as u16 - 1;
        self.x_counter = 0;
        self.y_counter = 0;
    }

    pub fn set_data_mode(&mut self, x_inc: bool, y_inc: bool) {
        self.x_dir = if x_inc { Dir::Increase } else { Dir::Decrease };
        self.y_dir = if y_inc { Dir::Increase } else { Dir::Decrease };
    }

    pub fn set_ram_x_range(&mut self, start: u16, end: u16) {
        self.x_start = start;
        self.x_end = end;
    }

    pub fn set_ram_y_range(&mut self, start: u16, end: u16) {
        self.y_start = start;
        self.y_end = end;
    }

    pub fn set_ram_x_counter(&mut self, val: u16) {
        self.x_counter = val;
    }

    pub fn set_ram_y_counter(&mut self, val: u16) {
        self.y_counter = val;
    }

    pub fn auto_write_ram(&mut self, red: bool, value: u8) {
        let ram = if red { &self.red_ram } else { &self.bw_ram };
        let ptr: *mut u8 = ram.as_ptr() as *mut u8;
        for i in 0..DISPLAY_BYTES {
            unsafe { *ptr.add(i) = value; }
        }
    }

    pub fn write_ram(&mut self, red: bool, data: &[u8]) {
        let ram = if red { &self.red_ram } else { &self.bw_ram };
        // SAFETY: we write only within bounds, and no other code reads these cells concurrently
        let ptr: *mut u8 = ram.as_ptr() as *mut u8;
        for &byte in data {
            let x = self.x_counter as usize;
            let y = self.y_counter as usize;
            let addr = y * DISPLAY_WIDTH / 8 + x / 8;
            if addr < DISPLAY_BYTES {
                unsafe { *ptr.add(addr) = byte; }
            }
            // Advance counters
            match self.x_dir {
                Dir::Increase => self.x_counter += 8,
                Dir::Decrease => self.x_counter = self.x_counter.wrapping_sub(8),
            }
            if self.x_counter > self.x_end || self.x_counter < self.x_start {
                self.x_counter = self.x_start;
                match self.y_dir {
                    Dir::Increase => {
                        self.y_counter += 1;
                        if self.y_counter > self.y_end {
                            self.y_counter = self.y_start;
                        }
                    }
                    Dir::Decrease => {
                        self.y_counter = self.y_counter.wrapping_sub(1);
                        if self.y_counter > self.y_start {
                            self.y_counter = self.y_start;
                        }
                    }
                }
            }
        }
    }

    pub fn read_ram(&mut self, red: bool) {
        let ram = if red { &self.red_ram } else { &self.bw_ram };
        let _ptr: *const u8 = ram.as_ptr() as *const u8;
        // Debug: print first 16 bytes
        // eprint!("Returned data ({}): ", if red { "Red" } else { "BW" });
        // for i in 0..16 { eprint!("{:02X}", unsafe { *_ptr.add(i) }); }
        // eprintln!("");
    }

    pub fn wait_while_busy(&mut self, _comment: &str) {}
}

// ── Text rendering helpers ──────────────────────────────────────────────────

/// Render the pixel buffer to a String using half-block Unicode characters.
/// Each character represents 1 column × 2 rows:
///   ' ' = both white
///   ▀   = top black, bottom white
///   ▄   = top white, bottom black
///   █   = both black
fn render_text(pixels: &[u8; DISPLAY_BYTES], crop_x: usize, crop_y: usize, crop_w: usize, crop_h: usize) -> String {
    let mut out = String::new();
    // Iterate rows in pairs (2 rows per char)
    let mut row = crop_y;
    while row < crop_y + crop_h {
        for col in 0..crop_w {
            let px_col = crop_x + col;
            if px_col >= DISPLAY_WIDTH { continue; }

            let top_idx = row * DISPLAY_WIDTH + px_col;
            let bot_idx = if row + 1 < crop_y + crop_h && row + 1 < DISPLAY_HEIGHT {
                (row + 1) * DISPLAY_WIDTH + px_col
            } else {
                top_idx
            };

            let top_byte = top_idx / 8;
            let top_bit = top_idx % 8;
            let bot_byte = bot_idx / 8;
            let bot_bit = bot_idx % 8;

            let top_white = (pixels[top_byte] & (0x80 >> top_bit)) == 0;
            let bot_white = (pixels[bot_byte] & (0x80 >> bot_bit)) == 0;

            let ch = match (top_white, bot_white) {
                (true,  true)  => ' ',
                (false, true)  => '▀',
                (true,  false) => '▄',
                (false, false) => '█',
            };
            out.push(ch);
        }
        out.push('\n');
        row += 2;
    }
    out
}

/// Crop region covering all SideTop squares after rotation.
fn side_top_crop() -> (usize, usize, usize, usize) {
    // SideTop positions in app: x=80,160,240,320,400, y=400
    // After rotation: display X = app Y = 400
    //                  display Y = 480 - 1 - app_x
    // For x=80:  display Y = 399
    // For x=400: display Y = 79
    // So Y ranges from ~79 to ~399, plus padding = 40..420
    // X is fixed at 400, so crop X = 380..440 = 60 pixels wide
    (380, 30, 70, 420)
}

// ── Display wrapper (mirrors the real xtx4_esp32::display::Display) ─────────

pub struct MockDisplay {
    controller: MockSsd1677,
    width: u16,
    height: u16,
    ram_region: Option<Rectangle>,
}

impl MockDisplay {
    pub fn new(width: u16, height: u16) -> Self {
        let mut d = Self {
            controller: MockSsd1677::new(),
            width,
            height,
            ram_region: None,
        };
        d.init();
        d
    }

    pub fn full_display_rect(&self) -> Rectangle {
        Rectangle { x: 0, y: 0, w: self.width, h: self.height }
    }

    fn init(&mut self) {
        println!("Initializing mock SSD1677...");
        self.controller.soft_reset();
        self.controller.auto_write_ram(false, 0xF7);
        self.controller.auto_write_ram(true, 0xF7);
        println!("Mock SSD1677 ready");
    }

    pub fn read_buffer(&mut self, color: Color) {
        let red = color == Color::Red;
        self.set_ram_area_intern(&self.full_display_rect());
        self.controller.read_ram(red);
    }

    pub fn set_ram_area(&mut self, region: &Rectangle) {
        self.ram_region = None;
        self.set_ram_area_intern(region);
        self.ram_region = Some(region.clone());
    }

    fn set_ram_area_intern(&mut self, region: &Rectangle) {
        let Rectangle { x, y, w, h } = *region;
        let y = self.height - y - h;

        self.controller.set_data_mode(true, false); // X inc, Y dec

        self.controller.set_ram_x_range(x, x + w - 1);
        self.controller.set_ram_x_counter(x);

        let y_start = y + h - 1;
        let y_end = y;
        self.controller.set_ram_y_range(y_start, y_end);
        self.controller.set_ram_y_counter(y_start);
    }

    pub fn write_region(&mut self, color: Color, buffer: &Buffer, rect: &Rectangle) {
        let Rectangle { w, h, .. } = rect;
        let bytes_to_write = (*w as usize) * (*h as usize) / 8;
        if bytes_to_write != buffer.as_slice_of_cells().len() {
            panic!(
                "Incorrect size for region write! Expected: {}, got: {}",
                buffer.as_slice_of_cells().len(),
                bytes_to_write
            );
        }
        self.set_ram_area(rect);
        let len = buffer.as_slice_of_cells().len();
        let data: &[u8] = unsafe { core::slice::from_raw_parts(buffer.as_ptr() as *const u8, len) };
        self.controller.write_ram(color == Color::Red, data);
    }

    pub fn refresh_full(&mut self) {
        self.controller.wait_while_busy("full refresh");
    }

    pub fn refresh_partial(&mut self) {
        self.controller.wait_while_busy("partial refresh");
    }

    /// Iterate over the raw BW pixel buffer bytes
    pub fn pixel_bytes(&self) -> impl Iterator<Item = u8> + '_ {
        let ptr: *const u8 = self.controller.bw_ram.as_ptr() as *const u8;
        (0..DISPLAY_BYTES).map(move |i| unsafe { *ptr.add(i) })
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Color {
    BlackWhite,
    Red,
}

// ── Emulated platform ───────────────────────────────────────────────────────

pub struct EmulatedPlatform {
    display: MockDisplay,
    window: Window,
    render_buf: Vec<u32>,
}

const BLACK: u32 = 0x001C1C1A;
const WHITE: u32 = 0x00F0ECD8;

impl EmulatedPlatform {
    pub fn new() -> Self {
        let window = Window::new(
            "Xteink X4 — Emulated Display",
            DISPLAY_WIDTH,
            DISPLAY_HEIGHT,
            WindowOptions::default(),
        )
        .expect("Failed to open emulator window");

        let mut s = Self {
            display: MockDisplay::new(DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16),
            window,
            render_buf: vec![WHITE; DISPLAY_WIDTH * DISPLAY_HEIGHT],
        };
        s.render_window();
        s.render_text();
        s
    }

    fn render_window(&mut self) {
        let pixels = self.display.pixel_bytes();
        for (i, byte) in pixels.enumerate() {
            for bit in 0..8 {
                let px = i * 8 + bit;
                if px < self.render_buf.len() {
                    let white = (byte & (0x80 >> bit)) != 0;
                    self.render_buf[px] = if white { WHITE } else { BLACK };
                }
            }
        }
        self.window
            .update_with_buffer(&self.render_buf, DISPLAY_WIDTH, DISPLAY_HEIGHT)
            .unwrap();
    }

    fn render_text(&self) {
        let mut pixel_buf = [0u8; DISPLAY_BYTES];
        for (i, b) in self.display.pixel_bytes().enumerate() {
            pixel_buf[i] = b;
        }
        let (cx, cy, cw, ch) = side_top_crop();
        let text = render_text(&pixel_buf, cx, cy, cw, ch);
        println!("┌─ SideTop crop ({},{}) {}×{} ───────────────────────┐", cx, cy, cw, ch);
        for line in text.lines() {
            if line.len() > 0 {
                println!("│{}│", line);
            }
        }
        println!("└──────────────────────────────────────────────────────┘");
    }

    fn push_display(&mut self, fb: &Framebuffer, full: bool) {
        let full_rect = self.display.full_display_rect();
        self.display.write_region(Color::BlackWhite, fb, &full_rect);
        if full {
            self.display.refresh_full();
            // Sync RED after full refresh
            self.display.write_region(Color::Red, fb, &full_rect);
        } else {
            self.display.refresh_partial();
            // Sync RED for next diff
            self.display.write_region(Color::Red, fb, &full_rect);
        }
        self.render_window();
        self.render_text();
    }
}

impl Platform for EmulatedPlatform {
    fn display_flush(&mut self, fb: &Framebuffer) {
        self.push_display(fb, true);
        self.display.read_buffer(Color::BlackWhite);
        self.display.read_buffer(Color::Red);
    }

    fn display_fast(&mut self, fb: &Framebuffer) {
        let full_rect = self.display.full_display_rect();
        self.display.write_region(Color::BlackWhite, fb, &full_rect);
        self.display.refresh_partial();
        self.display.write_region(Color::Red, fb, &full_rect);
        self.render_window();
        self.render_text();
    }

    fn display_flush_partial(&mut self, fb: &Buffer, frame: &Rectangle) {
        let Rectangle { x, y, w, h } = *frame;
        // Transform for display rotation (same as Esp32Platform)
        let frame = &Rectangle {
            x: y,
            y: FRAME_WIDTH as u16 - x - w,
            w: h,
            h: w,
        };
        self.display.write_region(Color::BlackWhite, fb, frame);
        self.display.refresh_partial();
        self.display.write_region(Color::Red, fb, frame);
        self.render_window();
        self.render_text();
    }

    fn button_state(&mut self) -> Buttons {
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
        self.window.update();
    }

    fn low_power_enable(&mut self) {}
    fn low_power_disable(&mut self) {}

    fn power_off(&mut self) {
        std::process::exit(0);
    }

    fn log(&mut self, msg: &str) {
        println!("{}", msg);
    }
}
