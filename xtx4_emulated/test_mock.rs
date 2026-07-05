// Standalone test: verify MockSsd1677 write_ram → pixel_bytes round trip.
// Compile: rustc --edition 2021 test_mock.rs -o test_mock && ./test_mock

// Copy the relevant types inline to avoid crate dependencies
const DISPLAY_WIDTH: usize = 800;
const DISPLAY_HEIGHT: usize = 480;
const DISPLAY_BYTES: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT / 8; // 48000

use std::cell::Cell;

enum Dir { Increase, Decrease }

struct MockSsd1677 {
    bw_ram: Cell<[u8; DISPLAY_BYTES]>,
    red_ram: Cell<[u8; DISPLAY_BYTES]>,
    x_start: u16,
    x_end: u16,
    x_counter: u16,
    y_start: u16,
    y_end: u16,
    y_counter: u16,
    x_dir: Dir,
    y_dir: Dir,
}

impl MockSsd1677 {
    fn new() -> Self {
        Self {
            bw_ram: Cell::new([0x00; DISPLAY_BYTES]),
            red_ram: Cell::new([0x00; DISPLAY_BYTES]),
            x_start: 0,
            x_end: DISPLAY_WIDTH as u16 - 1,
            x_counter: 0,
            y_start: 0,
            y_end: DISPLAY_HEIGHT as u16 - 1,
            y_counter: 0,
            x_dir: Dir::Increase,
            y_dir: Dir::Decrease,
        }
    }

    fn set_data_mode(&mut self, x_inc: bool, y_inc: bool) {
        self.x_dir = if x_inc { Dir::Increase } else { Dir::Decrease };
        self.y_dir = if y_inc { Dir::Increase } else { Dir::Decrease };
    }

    fn set_ram_x_range(&mut self, start: u16, end: u16) {
        self.x_start = start;
        self.x_end = end;
    }

    fn set_ram_x_counter(&mut self, start: u16) {
        self.x_counter = start;
    }

    fn set_ram_y_range(&mut self, start: u16, end: u16) {
        self.y_start = start;
        self.y_end = end;
    }

    fn set_ram_y_counter(&mut self, start: u16) {
        self.y_counter = start;
    }

    fn write_ram(&mut self, red: bool, data: &[u8]) {
        let ram = if red { &self.red_ram } else { &self.bw_ram };
        let ptr: *mut u8 = ram.as_ptr() as *mut u8;
        for &byte in data {
            let x = self.x_counter as usize;
            let y = self.y_counter as usize;
            let addr = y * DISPLAY_WIDTH / 8 + x / 8;
            if addr < DISPLAY_BYTES {
                unsafe { *ptr.add(addr) = byte; }
            }
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

    fn set_ram_area(&mut self, x: u16, y: u16, w: u16, h: u16) {
        // X increment, Y decrement
        self.set_data_mode(true, false);
        self.set_ram_x_range(x, x + w - 1);
        self.set_ram_x_counter(x);

        // Inverted Y
        let y_ram = DISPLAY_HEIGHT as u16 - y - h;
        let y_start = y_ram + h - 1;
        let y_end = y_ram;
        self.set_ram_y_range(y_start, y_end);
        self.set_ram_y_counter(y_start);
    }

    fn pixel_bytes(&self) -> impl Iterator<Item = u8> + '_ {
        let ptr: *const u8 = self.bw_ram.as_ptr() as *const u8;
        (0..DISPLAY_BYTES).map(move |i| unsafe { *ptr.add(i) })
    }
}

fn main() {
    let mut ssd = MockSsd1677::new();

    // Test 1: Write full display with 0xFF
    println!("=== Test 1: Write full display with 0xFF ===");
    let mut data = vec![0u8; DISPLAY_BYTES];
    data.fill(0xFF);
    ssd.set_ram_area(0, 0, DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16);
    ssd.write_ram(false, &data);

    let top_byte = ssd.pixel_bytes().next().unwrap();
    println!("Byte 0 = 0x{:02X} (expected 0xFF)", top_byte);
    assert_eq!(top_byte, 0xFF, "Full 0xFF write failed: byte 0 is 0x{:02X}", top_byte);

    // Test 2: Write full display with 0x00
    println!("=== Test 2: Write full display with 0x00 ===");
    data.fill(0x00);
    ssd.set_ram_area(0, 0, DISPLAY_WIDTH as u16, DISPLAY_HEIGHT as u16);
    ssd.write_ram(false, &data);

    let top_byte = ssd.pixel_bytes().next().unwrap();
    println!("Byte 0 = 0x{:02X} (expected 0x00)", top_byte);
    assert_eq!(top_byte, 0x00, "Full 0x00 write failed: byte 0 is 0x{:02X}", top_byte);

    // Test 3: Write a partial region (40x40 at display x=400, y=359)
    println!("=== Test 3: Partial 40x40 at (400, 359) ===");
    let region_size = 40u16;
    let nbytes = (region_size as usize * region_size as usize) / 8; // 200
    let mut partial = vec![0u8; nbytes];
    partial.fill(0xFF);
    ssd.set_ram_area(400, 359, region_size, region_size);
    ssd.write_ram(false, &partial);

    // The partial region should have set bytes 0xFF at (x=400..439, y=359..398 in display coords)
    // In RAM coords: y_ram = 480 - 359 - 40 = 81, so RAM rows 81..120
    // Byte address for (y=120, x=400): 120*100 + 50 = 12050
    // Byte address for (y=81, x=439): 81*100 + 54 = 8154
    // Byte 0 of buffer = RAM[12050] = covers display (x=400, y=359..365)... etc
    let mut bytes: Vec<u8> = ssd.pixel_bytes().collect();
    
    // Check: byte at RAM row 120, col-byte 50 should be 0xFF
    let addr_120_50 = 120 * 100 + 50;
    println!("RAM[{}] = 0x{:02X} (expected 0xFF)", addr_120_50, bytes[addr_120_50]);
    assert_eq!(bytes[addr_120_50], 0xFF, "Partial write not visible at expected address");
    
    // Check: byte at RAM row 0, col-byte 0 should still be 0x00 (outside region)
    println!("RAM[0] = 0x{:02X} (expected 0x00, outside region)", bytes[0]);
    assert_eq!(bytes[0], 0x00, "Area outside partial region was modified");

    println!("\n=== ALL TESTS PASSED ===");
}
