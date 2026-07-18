#![cfg_attr(target_arch = "riscv32", no_std)]

// ESP32-C3 hardware transport for the SSD1677 display.
// Uses an embedded-hal-bus CriticalSectionDevice backed by the shared
// xtx4_bus global, so the SPI bus is safely shared with the SD card.

use embedded_hal::spi::SpiDevice;
use embedded_hal_bus::spi::CriticalSectionDevice;
use esp_hal::{
    delay::Delay,
    gpio::{Input, Output},
    spi::master::Spi,
};
use esp_println::{println, print};
use ssd1677::DisplayInterface;
use xtx4_host;

type SpiDev = CriticalSectionDevice<'static, Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>;

pub struct EspInterface {
    spi: SpiDev,
    dc:  Output<'static>,
    rst: Output<'static>,
    busy: Input<'static>,
}

impl EspInterface {
    pub fn new(spi: SpiDev, dc: Output<'static>, rst: Output<'static>, busy: Input<'static>) -> Self {
        Self { spi, dc, rst, busy }
    }
}

impl DisplayInterface for EspInterface {
    fn write_command(&mut self, cmd: u8) {
        let t = self.millis();
        println!("[{}] CMD 0x{:02X}", t, cmd);
        self.dc.set_low();
        self.spi.write(&[cmd]).unwrap();
    }

    fn write_data(&mut self, data: &[u8]) {
        let t = self.millis();
        if data.len() < 32 {
            print!("[{}] DATA [", t);
            for (i, b) in data.iter().enumerate() {
                if i > 0 { print!(" "); }
                print!("{:02X}", b);
            }
            let unit = if data.len() == 1 { "byte" } else { "bytes" };
            println!("] ({} {})", data.len(), unit);
        } else {
            print!("[{}] DATA [", t);
            for i in 0..16 {
                if i > 0 { print!(" "); }
                print!("{:02X}", data[i]);
            }
            let unit = if data.len() == 1 { "byte" } else { "bytes" };
            println!("...] ({} {})", data.len(), unit);
        }
        self.dc.set_high();
        if !data.is_empty() {
            self.spi.write(data).unwrap();
        }
    }

    fn read_data(&mut self, data: &mut [u8]) {
        let t = self.millis();
        println!("[{}] READ {} bytes", t, data.len());
        self.dc.set_high();
        if !data.is_empty() {
            self.spi.transfer_in_place(data).unwrap();
        }
    }

    fn reset(&mut self) {
        let t = xtx4_host::now_ms();
        println!("[{}] RESET", t);
        self.rst.set_high();
        xtx4_host::delay_ms(20);
        self.rst.set_low();
        xtx4_host::delay_ms(2);
        self.rst.set_high();
        xtx4_host::delay_ms(20);
    }

    fn busy_high(&self) -> bool {
        self.busy.is_high()
    }
}

impl EspInterface {
    fn millis(&self) -> u32 {
        xtx4_host::now_ms()
    }
}
