use esp_hal::{
    prelude::*,
    peripherals,
    spi,
    spi::master::Spi,
    spi::SpiMode,
    gpio::{Level, Output, AnyPin},
};
use embassy_embedded_hal::SetConfig;
use xtx4_platform_interface::{Buffer};

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

pub struct SSD1677Builder {
    pub spi: peripherals::SPI2, 
    pub sck: AnyPin,
    pub mosi: AnyPin,
    pub cs: AnyPin,
    pub dc: AnyPin,
}

pub struct SSD1677 {
    spi:          Spi<'static, esp_hal::Blocking>,
    cs:           Output<'static>,
    dc:           Output<'static>,
}

impl SSD1677 {
    pub fn new(peripherals: SSD1677Builder) -> Self {
        let mut spi = Spi::new(peripherals.spi)
            .with_sck(peripherals.sck)
            .with_mosi(peripherals.mosi);
        spi.set_config(&spi::master::Config {
                frequency: 40u32.MHz(),
                mode: SpiMode::Mode0,
                ..spi::master::Config::default()
            }).unwrap();

        let cs   = Output::new(peripherals.cs, Level::High);
        let dc   = Output::new(peripherals.dc,  Level::High);

        Self {
            spi,
            cs,
            dc,
        }
    }

    pub fn write_ram(&mut self, color: Color, buffer: &Buffer) {
        let command = match color {
            Color::Red => SSD1677Command::WriteRamRed,
            Color::BlackWhite => SSD1677Command::WriteRamBw,
        };

        self.send_command(command);
        self.send_data(buffer);
    }

    pub fn set_data_mode(&mut self, x: DataEntryMode, y: DataEntryMode) {
        let x = if x == DataEntryMode::Increase { 0x1 } else { 0x0 };
        let y = if y == DataEntryMode::Increase { 0x2 } else { 0x0 };
        let val: u8 = x | y;

        self.send_command(SSD1677Command::DataEntryMode);
        self.send_byte(val);
    }

    pub fn send_command(&mut self, cmd: SSD1677Command) {
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write_bytes(&[cmd as u8]).unwrap();
        self.cs.set_high();
    }

    pub fn send_byte(&mut self, data: u8) {
        self.dc.set_high();
        self.cs.set_low();
        self.spi.write_bytes(&[data]).unwrap();
        self.cs.set_high();
    }

    pub fn send_data(&mut self, data: &Buffer) {
        self.dc.set_high();
        self.cs.set_low();

        // SAFETY: read-only, fixed lifetime use
        let data: &[u8] = unsafe { &*(data.as_ptr()) };
        self.spi.write_bytes(data).unwrap();

        self.cs.set_high();
    }
}
