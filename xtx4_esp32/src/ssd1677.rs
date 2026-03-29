use esp_hal::{
    time::Rate,
    spi::{
        Mode,
        master::{Spi, Config, AnySpi},
    },
    gpio::{Level, Pull, Input, Output, AnyPin},
};
use esp_hal::gpio::InputConfig;
use esp_hal::gpio::OutputConfig;
use esp_println::println;
use xtx4_platform_interface::{Buffer};
use bitflags::bitflags;

use crate::sleep::sleep_ms;

const CTRL1_BYPASS_RED: u8 = 0x40;

pub enum SSD1677Command {
    DriverOutputControl = 0x01,
    BoosterSoftStart    = 0x0C,
    DataEntryMode       = 0x11,
    SoftReset           = 0x12,
    TempSensorControl   = 0x18,
    BorderWaveform      = 0x3C,

    SetRamXRange        = 0x44,
    SetRamYRange        = 0x45,
    SetRamXCounter      = 0x4E,
    SetRamYCounter      = 0x4F,

    MasterActivation    = 0x20,
    DisplayUpdateCtrl1  = 0x21,
    DisplayUpdateCtrl2  = 0x22,

    WriteRamBw          = 0x24,
    WriteRamRed         = 0x26,

    AutoWriteBwRam      = 0x46,
    AutoWriteRedRam     = 0x47,
}


#[derive(PartialEq, Eq)]
pub enum DataEntryMode {
    Increase,
    Decrease,
}

pub enum Color {
    Red,
    BlackWhite,
}

pub enum Range {
    X,
    Y,
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct DriverOutputControlMode: u8 {
        const TB = 1 << 0; // Always 0
        const SM = 1 << 1; // Scan order, 0 left and right interlaced, 1 no splitting.
        const GD  = 1 << 2; // Set starting line, see table in manual.
    }
}

pub struct SSD1677Builder {
    pub spi: AnySpi<'static>, 
    pub sck: AnyPin<'static>,
    pub mosi: AnyPin<'static>,
    pub cs: AnyPin<'static>,
    pub dc: AnyPin<'static>,
    pub rst: AnyPin<'static>,
    pub busy: AnyPin<'static>,
}

pub struct SSD1677 {
    spi:          Spi<'static, esp_hal::Blocking>,
    cs:           Output<'static>,
    dc:           Output<'static>,
    rst:          Output<'static>,
    busy:         Input<'static>,
    is_screen_on: bool,
}

fn split_bytes(value: u16) -> (u8, u8) {
    const MAX_BYTE : u16 = 1<<8;
    ((value / MAX_BYTE) as u8, (value % MAX_BYTE) as u8)
}

impl SSD1677 {
    pub fn new(peripherals: SSD1677Builder) -> Self {
        let config = Config::default()
            .with_frequency(Rate::from_mhz(40u32))
            .with_mode(Mode::_0);

        let spi = Spi::new(peripherals.spi, config)
            .expect("SPI failed to initialise")
            .with_sck(peripherals.sck)
            .with_mosi(peripherals.mosi);

        let out_config = OutputConfig::default();
        let cs   = Output::new(peripherals.cs, Level::High, out_config);
        let dc   = Output::new(peripherals.dc, Level::High, out_config);
        let rst  = Output::new(peripherals.rst, Level::High, out_config);

        let busy_config = InputConfig::default().with_pull(Pull::None);
        let busy = Input::new(peripherals.busy, busy_config);

        Self {
            spi,
            cs,
            dc,
            rst,
            busy,
            is_screen_on: false,
        }
    }

    pub fn reset(&mut self) {
        self.rst.set_high();
        sleep_ms(20);
        self.rst.set_low();
        sleep_ms(2);
        self.rst.set_high();
        sleep_ms(20);
    }

    pub fn soft_reset(&mut self) {
        self.send_command(SSD1677Command::SoftReset);
        self.wait_while_busy("soft reset");
    }

    pub fn set_temp_sensor(&mut self, sensor: u8) {
        self.send_command(SSD1677Command::TempSensorControl);
        self.send_byte(sensor);
    }

    pub fn booster_soft_start(&mut self, sequence: &Buffer) {
        self.send_command(SSD1677Command::BoosterSoftStart);
        self.send_data(sequence);
    }

    pub fn driver_output_control(&mut self, height: u16, mode: DriverOutputControlMode) {
        const HEIGHT_MAX : u16 = 1<<10; // 10 bits MUX from manual
                               //
        if height >= HEIGHT_MAX {
            panic!("Tried to set driver output with {}, max height is {}", height, HEIGHT_MAX);
        }

        let max_byte = 1 << 8; // Bits per byte.
        let mux = height - 1; // Turn into flags for mux.
        let lower_bits = (mux % max_byte) as u8;
        let upper_bits = (mux / max_byte) as u8;

        self.send_command(SSD1677Command::DriverOutputControl);
        self.send_byte(lower_bits);
        self.send_byte(upper_bits);
        self.send_byte(mode.bits()); // SM=1
    }

    pub fn set_border_waveform(&mut self, mode: u8) {
        self.send_command(SSD1677Command::BorderWaveform);
        self.send_byte(mode);
    }

    pub fn auto_write_ram(&mut self, color: Color, value: u8) {
        let command = match color {
            Color::BlackWhite => SSD1677Command::AutoWriteBwRam,
            Color::Red => SSD1677Command::AutoWriteRedRam,
        };

        self.send_command(command);
        self.send_byte(value);
        match color {
            Color::BlackWhite => self.wait_while_busy("auto write BW RAM"),
            Color::Red => self.wait_while_busy("auto write Red RAM"),
        }
    }

    pub fn display_update_ctrl1(&mut self, command: u8) {
        self.send_command(SSD1677Command::DisplayUpdateCtrl1);
        self.send_byte(command);
    }

    pub fn display_update_ctrl2(&mut self, command: u8) {
        self.send_command(SSD1677Command::DisplayUpdateCtrl2);
        self.send_byte(command);
    }

    pub fn master_activation(&mut self) {
        self.send_command(SSD1677Command::MasterActivation);
        self.wait_while_busy("master activation");
    }

    fn wait_while_busy(&mut self, comment: &str) {
        let mut timeout = 10_000u32;
        while self.busy.is_high() {
            sleep_ms(1u32);
            timeout -= 1;
            if timeout == 0 {
                println!("Timeout waiting for busy: {}", comment);
                return;
            }
        }
        println!("Ready: {}", comment);
    }

    pub fn refresh_full(&mut self) {
        self.display_update_ctrl1(CTRL1_BYPASS_RED);

        let mut mode = 0x34u8;
        if !self.is_screen_on {
            self.is_screen_on = true;
            mode |= 0xC0; // CLOCK_ON + ANALOG_ON
        }

        self.display_update_ctrl2(mode);
        self.master_activation();
    }

    pub fn write_ram(&mut self, color: Color, buffer: &Buffer) {
        let command = match color {
            Color::Red => SSD1677Command::WriteRamRed,
            Color::BlackWhite => SSD1677Command::WriteRamBw,
        };

        self.send_command(command);
        self.send_data(buffer);
    }

    pub fn set_ram_range(&mut self, range: Range, start: u16, end: u16) {
        let command = match range {
            Range::X => SSD1677Command::SetRamXRange,
            Range::Y => SSD1677Command::SetRamYRange,
        };
        self.send_command(command);

        let (s_upper, s_lower) = split_bytes(start);
        self.send_byte(s_lower);
        self.send_byte(s_upper);

        let (e_upper, e_lower) = split_bytes(end);
        self.send_byte(e_lower);
        self.send_byte(e_upper);
    }

    pub fn set_ram_counter(&mut self, range: Range, val: u16) {
        let command = match range {
            Range::X => SSD1677Command::SetRamXCounter,
            Range::Y => SSD1677Command::SetRamYCounter,
        };

        self.send_command(command);

        let (o_upper, o_lower) = split_bytes(val);
        self.send_byte(o_lower);
        self.send_byte(o_upper);
    }

    pub fn set_data_mode(&mut self, x: DataEntryMode, y: DataEntryMode) {
        let x = if x == DataEntryMode::Increase { 0x1 } else { 0x0 };
        let y = if y == DataEntryMode::Increase { 0x2 } else { 0x0 };
        let val: u8 = x | y;

        self.send_command(SSD1677Command::DataEntryMode);
        self.send_byte(val);
    }

    fn send_command(&mut self, cmd: SSD1677Command) {
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write(&[cmd as u8]).unwrap();
        self.cs.set_high();
    }

    fn send_byte(&mut self, data: u8) {
        self.dc.set_high();
        self.cs.set_low();
        self.spi.write(&[data]).unwrap();
        self.cs.set_high();
    }

    fn send_data(&mut self, data: &Buffer) {
        self.dc.set_high();
        self.cs.set_low();

        // SAFETY: read-only, fixed lifetime use
        let data: &[u8] = unsafe { &*(data.as_ptr()) };
        self.spi.write(data).unwrap();

        self.cs.set_high();
    }
}
