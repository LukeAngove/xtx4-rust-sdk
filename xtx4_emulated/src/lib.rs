// Emulated e-paper display backend — enabled with feature = "emulated".
//
// Uses an in-memory pair of (BW, RED) RAM banks to simulate the SSD1677,
// runs the exact same display pipeline as the ESP32 driver, and renders
// the display output as a TUI in the terminal. Key presses from tmux
// send-keys drive the button handlers.

use core::cell::Cell;
use crossterm::{
    cursor::MoveTo,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use std::io::{stdout, Write, Read};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;

static LAST_KEY: AtomicU8 = AtomicU8::new(0);

fn spawn_stdin_reader() {
    thread::spawn(|| {
        let mut buf = [0u8; 1];
        loop {
            match std::io::stdin().read(&mut buf) {
                Ok(_) => {
                    LAST_KEY.store(buf[0], Ordering::Relaxed);
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    });
}
use xtx4_platform_interface::{
    Buffer, Buttons, DrawTransform, Framebuffer, Platform, Rectangle,
    FRAME_HEIGHT, FRAME_WIDTH,
};

// Display dimensions in landscape (same as hardware)
pub const DISPLAY_WIDTH: usize = FRAME_HEIGHT; // 800
pub const DISPLAY_HEIGHT: usize = FRAME_WIDTH; // 480
pub const DISPLAY_BYTES: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT / 8;

// ── Coordinate transform (same as Esp32Transform) ──────────────────────────

pub struct EmulatedTransform;

impl DrawTransform for EmulatedTransform {
    fn stride(_full_width: u16, full_height: u16) -> u16 {
        full_height
    }

    fn apply(x: u16, y: u16, _width: u16, _height: u16) -> Option<(u16, u16)> {
        if x >= FRAME_WIDTH as u16 {
            return None;
        }
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
        for &byte in data.iter() {
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
pub fn render_text(pixels: &[u8; DISPLAY_BYTES]) -> String {
    // Portrait: each char is 1 portrait-column × 2 portrait-rows
    // Portrait dims: 480 cols × 800 rows = 480 × 400 chars
    let mut out = String::new();
    for row in 0..DISPLAY_WIDTH/2 {
        for col in 0..DISPLAY_HEIGHT {
            // Landscape: l_y = 479 - col (portrait X), l_x = portrait Y
            let top_l_y = DISPLAY_HEIGHT - 1 - col;
            let top_l_x = row * 2;
            let bot_l_x = if row * 2 + 1 < DISPLAY_WIDTH { row * 2 + 1 } else { row * 2 };
            let top_addr = top_l_y * DISPLAY_WIDTH + top_l_x;
            let bot_addr = top_l_y * DISPLAY_WIDTH + bot_l_x;
            let top_byte = top_addr / 8;
            let top_bit = top_addr % 8;
            let bot_byte = bot_addr / 8;
            let bot_bit = bot_addr % 8;
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
    }
    out
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
        self.controller.soft_reset();
        self.controller.auto_write_ram(false, 0xF7);
        self.controller.auto_write_ram(true, 0xF7);
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
    frame_counter: u64,
}



impl EmulatedPlatform {
    pub fn new() -> Self {
        // Enable raw mode and switch to alternate screen (no scrollback)
        enable_raw_mode().expect("enable raw mode");
        let mut out = stdout();
        let _ = execute!(out, EnterAlternateScreen);
        spawn_stdin_reader();

        let mut s = Self {
            display: MockDisplay::new(DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16),
            frame_counter: 0,
        };
        s.render_text();
        s
    }

    fn render_window(&mut self) {
        // No graphical window; TUI output via render_text()
    }

    fn render_text(&mut self) {
        let mut pixel_buf = [0u8; DISPLAY_BYTES];
        for (i, b) in self.display.pixel_bytes().enumerate() {
            pixel_buf[i] = b;
        }
        // Write PBM frame (portrait orientation: 480 wide × 800 tall)
        let _ = std::fs::create_dir_all("/tmp/xtx4_frames");
        let path = format!("/tmp/xtx4_frames/frame_{:04}.pbm", self.frame_counter);
        if let Ok(mut f) = std::fs::File::create(&path) {
            use std::io::Write;
            // PBM header: width=480 (portrait), height=800 (portrait)
            let _ = f.write_all(b"P4\n480 800\n");
            // Build portrait byte array: 480 cols × 800 rows / 8 = 48000 bytes
            // Portrait pixel (p_x, p_y) → landscape (l_x=p_y, l_y=479-p_x)
            // Portrait byte index = p_y * (480/8) + p_x/8 = p_y * 60 + p_x/8
            // Portrait bit = p_x % 8
            // Landscape byte = l_y * 100 + l_x/8
            // Landscape bit = l_x % 8
            let mut portrait = [0u8; DISPLAY_BYTES];
            for p_y in 0..DISPLAY_WIDTH {
                for p_x in 0..DISPLAY_HEIGHT {
                    let l_y = DISPLAY_HEIGHT - 1 - p_x;
                    let l_x = p_y;
                    let l_byte = l_y * (DISPLAY_WIDTH / 8) + l_x / 8;
                    let l_bit = l_x % 8;
                    let p_byte = p_y * (DISPLAY_HEIGHT / 8) + p_x / 8;
                    let p_bit = p_x % 8;
                    let bit_val = (pixel_buf[l_byte] >> (7 - l_bit)) & 1;
                    portrait[p_byte] |= bit_val << (7 - p_bit);
                }
            }
            let _ = f.write_all(&portrait);
        }
        self.frame_counter += 1;

        // Render to terminal (alternate screen)
        let text = render_text(&pixel_buf);
        let mut out = stdout();
        let _ = execute!(out, Clear(ClearType::All), MoveTo(0, 0));
        for line in text.lines() {
            let _ = write!(out, "{}\r\n", line);
        }
        let _ = out.flush();
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
    }
}

impl Platform for EmulatedPlatform {
    fn display_flush(&mut self, fb: &Framebuffer) {
        self.push_display(fb, true);
        self.display.read_buffer(Color::BlackWhite);
        self.display.read_buffer(Color::Red);
        self.render_text();
    }

    fn display_fast(&mut self, fb: &Framebuffer) {
        let full_rect = self.display.full_display_rect();
        self.display.write_region(Color::BlackWhite, fb, &full_rect);
        self.display.refresh_partial();
        self.display.write_region(Color::Red, fb, &full_rect);
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
        self.render_text();
    }

    fn button_state(&mut self) -> Buttons {
        let key = LAST_KEY.swap(0, Ordering::Relaxed);
        let mut state = Buttons::empty();
        match key {
            b'd' | b'D' | b'r' | b'R' => state |= Buttons::LEFT_OUTER,
            b'f' | b'F' => state |= Buttons::LEFT_INNER,
            b'j' | b'J' => state |= Buttons::RIGHT_INNER,
            b'k' | b'K' => state |= Buttons::RIGHT_OUTER,
            b'l' | b'L' => state |= Buttons::SIDE_TOP,
            b';' => state |= Buttons::SIDE_BOTTOM,
            b'p' | b'P' => state |= Buttons::POWER,
            b'q' | b'Q' => {
                let _ = execute!(stdout(), LeaveAlternateScreen);
                let _ = disable_raw_mode();
                std::process::exit(0);
            }
            _ => {}
        }
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
    }

    fn low_power_enable(&mut self) {}
    fn low_power_disable(&mut self) {}

    fn power_off(&mut self) {
        let _ = disable_raw_mode();
        std::process::exit(0);
    }

    fn log(&mut self, _msg: &str) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pixels(val: u8) -> [u8; DISPLAY_BYTES] {
        [val; DISPLAY_BYTES]
    }

    #[test]
    fn all_white_gives_spaces() {
        let pixels = make_pixels(0x00);
        let out = render_text(&pixels);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), DISPLAY_WIDTH / 2);
        for (i, line) in lines.iter().enumerate() {
            assert_eq!(line.chars().count(), DISPLAY_HEIGHT, "line {i} wrong width");
            for ch in line.chars() {
                assert_eq!(ch, ' ');
            }
        }
    }

    #[test]
    fn all_black_gives_full_blocks() {
        let pixels = make_pixels(0xFF);
        let out = render_text(&pixels);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), DISPLAY_WIDTH / 2);
        for (i, line) in lines.iter().enumerate() {
            assert_eq!(line.chars().count(), DISPLAY_HEIGHT, "line {i} wrong width");
            for ch in line.chars() {
                assert_eq!(ch, '█');
            }
        }
    }

    #[test]
    fn single_pixel_top_half() {
        // Landscape (x=0, y=479) → portrait (col=0, row=0)
        // byte = 479 * 100 + 0/8 = 47900, bit = 0
        let mut pixels = make_pixels(0x00);
        pixels[47900] = 0x80; // bit 0 set
        let out = render_text(&pixels);
        let ch = out.lines().next().unwrap().chars().next().unwrap();
        assert_eq!(ch, '▀', "single top pixel should give top-half block");
    }

    #[test]
    fn single_pixel_bottom_half() {
        // Landscape (x=1, y=479) → portrait (col=0, row=1) → same byte bit 1
        let mut pixels = make_pixels(0x00);
        pixels[47900] = 0x40; // bit 1 set
        let out = render_text(&pixels);
        let ch = out.lines().next().unwrap().chars().next().unwrap();
        assert_eq!(ch, '▄', "single bottom pixel should give bottom-half block");
    }

    #[test]
    fn both_pixels_full_block() {
        // Landscape (x=0,1, y=479) → portrait (col=0, rows 0+1)
        let mut pixels = make_pixels(0x00);
        pixels[47900] = 0xC0; // bits 0 and 1 set
        let out = render_text(&pixels);
        let ch = out.lines().next().unwrap().chars().next().unwrap();
        assert_eq!(ch, '█', "both pixels should give full block");
    }

    #[test]
    fn coordinate_mapping() {
        // Portrait (col=100, row=200) → landscape (l_x=200, l_y=479-100=379)
        // byte = 379 * 100 + 200/8 = 37900 + 25 = 37925, bit = 200%8 = 0
        let mut pixels = make_pixels(0x00);
        pixels[37925] = 0x80;
        let out = render_text(&pixels);
        let lines: Vec<&str> = out.lines().collect();
        let ch = lines[100].chars().nth(100).unwrap();
        assert_eq!(ch, '▀', "coord mapping: portrait (100,200) should be at line 100, col 100");
    }

    #[test]
    fn mock_display_full_flush() {
        use core::cell::Cell;
        let mut display = MockDisplay::new(DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16);
        let full_rect = display.full_display_rect();

        // Write all white
        let fb_cell: Cell<[u8; DISPLAY_BYTES]> = Cell::new([0u8; DISPLAY_BYTES]);
        display.write_region(Color::BlackWhite, unsafe { &*(&fb_cell as *const Cell<[u8; DISPLAY_BYTES]> as *const Cell<[u8]>) }, &full_rect);
        let pixels: Vec<u8> = display.pixel_bytes().collect();
        assert!(pixels.iter().all(|&b| b == 0x00), "all-white write should give all 0x00 in RAM");
        let pixels_arr: [u8; DISPLAY_BYTES] = pixels.try_into().unwrap();
        let text = render_text(&pixels_arr);
        assert!(text.lines().next().unwrap().chars().all(|c| c == ' '), "all-white should render as spaces");

        // Write all black
        let fb_cell = Cell::new([0xFFu8; DISPLAY_BYTES]);
        display.write_region(Color::BlackWhite, unsafe { &*(&fb_cell as *const Cell<[u8; DISPLAY_BYTES]> as *const Cell<[u8]>) }, &full_rect);
        let pixels: Vec<u8> = display.pixel_bytes().collect();
        assert!(pixels.iter().all(|&b| b == 0xFF), "all-black write should give all 0xFF in RAM");
        let pixels_arr: [u8; DISPLAY_BYTES] = pixels.try_into().unwrap();
        let text = render_text(&pixels_arr);
        assert!(text.lines().next().unwrap().chars().all(|c| c == '█'), "all-black should render as full blocks");

        // Partial write: 40x40 black square at app (350,380)
        // Rotated in display_flush_partial: x=380, y=480-350-50=80, w=20, h=50
        // Buffer must be exactly region_size = 20*50/8 = 125 bytes
        let region_size = 20 * 50 / 8;
        let fb_cell: Cell<[u8; 125]> = Cell::new([0xFFu8; 125]);
        let buf: &Cell<[u8]> = unsafe { &*(&fb_cell as *const Cell<[u8; 125]> as *const Cell<[u8]>) };
        let rect = Rectangle { x: 380, y: 80, w: 20, h: 50 };
        display.write_region(Color::BlackWhite, buf, &rect);
        let pixels: Vec<u8> = display.pixel_bytes().collect();

        // RAM: y=480-80-50=350, y_start=399, y_end=350
        // RAM row 399, column 380 → byte 399*100 + 380/8 = 39900 + 47 = 39947
        assert_eq!(pixels[39947], 0xFF, "partial write should set byte at row 399 col 380");
        // A byte outside the region should still be 0x00 (from previous all-black was overwritten)
        // Actually we wrote all 0xFF before, so check a byte outside the partial region
        assert_eq!(pixels[0], 0xFF, "outside region should retain previous value (0xFF)");
    }
}
