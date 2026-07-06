#[cfg(not(target_arch = "x86_64"))]
use esp_println::{println, print};
#[cfg(target_arch = "x86_64")]
use std::{println, print};
use xtx4_platform_interface::{Buffer};
use bitflags::bitflags;

use crate::display_transport::DisplayTransport;

bitflags! {
    #[derive(Clone, Copy)]
    pub struct DisplayUpdate1Commands : u8 {
        const Normal = 0;
        const BypassBw  = 0x04;
        const InvertBw  = 0x08;
        const BypassRed = 0x40;
        const InvertRed = 0x80;
    }
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct DisplayUpdate2Commands : u8 {
        const DisableClock  = 0x01;
        const DisableAnalog = 0x02;
        const DisplayEnable = 0x04;
        const DisplayMode1  = 0x00;
        const DisplayMode2  = 0x08;
        const LoadLUT       = 0x10;
        const LoadI2CTemp   = 0x20;
        const EnableAnalog  = 0x40;
        const EnableClock   = 0x80;
    }
}

#[derive(Debug)]
pub enum SSD1677Command {
    DriverOutputControl             = 0x01,
    GateDrivingVoltageControl       = 0x03,
    SourceDrivingVoltageControl     = 0x04,
    InitialCodeSettingOtpProgram    = 0x08,
    WriteRegisterInitialCodeSetting = 0x09,
    ReadRegisterInitialCodeSetting  = 0x0A,
    BoosterSoftStart                = 0x0C,
    DeepSleepMode                   = 0x10,
    DataEntryMode                   = 0x11,
    SoftReset                       = 0x12,
    HvReadyDetection                = 0x14,
    VciDetection                    = 0x15,

    TempSensorControl               = 0x18,
    TempSensorControlWrite          = 0x1A,
    TempSensorControlRead           = 0x1B,
    TemSensorControlWriteExternal   = 0x1C,

    MasterActivation                = 0x20,
    DisplayUpdateCtrl1              = 0x21,
    DisplayUpdateCtrl2              = 0x22,

    WriteRamBw                      = 0x24,
    WriteRamDithering               = 0x25, // Use 0x4D for settings
    WriteRamRed                     = 0x26,

    ReadRam                         = 0x27, // Use register 0x41 to select red or bw.

    VComSense                       = 0x28,
    VComSenseDuration               = 0x29,
    ProgramVComOtp                  = 0x2A,
    VComControlWrite                = 0x2B,
    VComRegisterWrite               = 0x2C,

    OtpDisplayRead                  = 0x2D,
    UserIdRead                      = 0x2E,
    StatusBitRead                   = 0x2F,

    WriteWaveformSettingOtp         = 0x30,
    LoadWaveformSettingOtp          = 0x31,

    WriteLutRegister                = 0x32,

    CrcCalculate                    = 0x34,
    CrcStatus                       = 0x35,

    WriteOtpSelection               = 0x36,
    DisplayOptionRegisterWrite      = 0x37,
    UserIdWrite                     = 0x38,
    WriteOptMode                    = 0x39,

    BorderWaveform                  = 0x3C,

    ReadRamOption                   = 0x41, // 0 = 0x24, 1 = 0x26

    SetRamXRange                    = 0x44,
    SetRamYRange                    = 0x45,

    AutoWriteBwRam                  = 0x46,
    AutoWriteRedRam                 = 0x47,

    DitheringEngine                 = 0x4D, // TODO See datasheet.

    SetRamXCounter                  = 0x4E,
    SetRamYCounter                  = 0x4F,
}


#[derive(PartialEq, Eq)]
pub enum DataEntryMode {
    Increase,
    Decrease,
}

#[derive(Debug)]
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

pub struct SSD1677<T: DisplayTransport> {
    pub transport:   T,
    is_screen_on: bool,
}

fn split_bytes(value: u16) -> (u8, u8) {
    const MAX_BYTE : u16 = 1<<8;
    ((value / MAX_BYTE) as u8, (value % MAX_BYTE) as u8)
}

impl<T: DisplayTransport> SSD1677<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            is_screen_on: false,
        }
    }

    pub fn reset(&mut self) {
        self.transport.reset();
    }

    pub fn soft_reset(&mut self) {
        self.send_command(SSD1677Command::SoftReset);
        self.wait_while_busy("soft reset");
    }

    pub fn set_temp_sensor(&mut self, sensor: u8) {
        self.send_command(SSD1677Command::TempSensorControl);
        self.send_byte(sensor);
    }

    pub fn write_temp_register(&mut self, temp: u8) {
        self.send_command(SSD1677Command::TempSensorControlWrite);
        self.send_byte(temp);
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

    pub fn display(&mut self, command1: DisplayUpdate1Commands, command2: DisplayUpdate2Commands, comment: &str) {
        self.display_update_ctrl1(command1);
        self.display_update_ctrl2(command2);
        self.master_activation(comment);
    }

    pub fn screen_sleep(&mut self) {
        //self.display(DisplayUpdate1Commands::Normal, DisplayUpdate2Commands::DisableClock | DisplayUpdate2Commands::DisableAnalog);
        //self.is_screen_on = false;
    }

    pub fn display_update_ctrl1(&mut self, command: DisplayUpdate1Commands) {
        self.send_command(SSD1677Command::DisplayUpdateCtrl1);
        self.send_byte(command.bits());
    }

    pub fn display_update_ctrl2(&mut self, command: DisplayUpdate2Commands) {
        self.send_command(SSD1677Command::DisplayUpdateCtrl2);
        self.send_byte(command.bits());
    }

    pub fn master_activation(&mut self, comment: &str) {
        self.send_command(SSD1677Command::MasterActivation);
        self.wait_while_busy(comment);
    }

    /// Block until BUSY goes low (or timeout).
    pub fn wait(&mut self, comment: &str) {
        self.wait_while_busy(comment);
    }

    /// Returns true if the display controller is busy.
    pub fn is_busy(&self) -> bool {
        self.transport.busy_high()
    }

    fn wait_while_busy(&mut self, comment: &str) {
        let start = self.transport.millis();
        if !self.transport.busy_high() {
            return;
        }
        println!("[{}] BUSY=1 (waiting: {})", start, comment);
        let mut timeout = 10_000u32;
        while self.transport.busy_high() {
            self.transport.delay_ms(1u32);
            timeout -= 1;
            if timeout == 0 {
                println!("Timeout waiting for busy: {}", comment);
                return;
            }
        }
        let done = self.transport.millis();
        println!("[{}] BUSY=0 (done: {}, {} ms)", done, comment, done - start);
    }

    pub fn refresh_full(&mut self) {
        let mut mode = DisplayUpdate2Commands::LoadI2CTemp | DisplayUpdate2Commands::LoadLUT | DisplayUpdate2Commands::DisplayMode1 | DisplayUpdate2Commands::DisplayEnable;
        if !self.is_screen_on {
            self.is_screen_on = true;
            mode |= DisplayUpdate2Commands::EnableClock | DisplayUpdate2Commands::EnableAnalog;
        }
        self.display(DisplayUpdate1Commands::BypassRed, mode, "full");
    }

    /// Send control registers + MasterActivation for full refresh, no busy wait.
    pub fn trigger_full(&mut self) {
        let mut mode = DisplayUpdate2Commands::LoadI2CTemp | DisplayUpdate2Commands::LoadLUT | DisplayUpdate2Commands::DisplayMode1 | DisplayUpdate2Commands::DisplayEnable;
        if !self.is_screen_on {
            self.is_screen_on = true;
            mode |= DisplayUpdate2Commands::EnableClock | DisplayUpdate2Commands::EnableAnalog;
        }
        self.display_update_ctrl1(DisplayUpdate1Commands::BypassRed);
        self.display_update_ctrl2(mode);
        self.send_command(SSD1677Command::MasterActivation);
    }

    pub fn refresh_partial(&mut self) {
        let mut mode = DisplayUpdate2Commands::LoadLUT | DisplayUpdate2Commands::DisplayMode2 | DisplayUpdate2Commands::DisplayEnable;
        if !self.is_screen_on {
            self.is_screen_on = true;
            mode |= DisplayUpdate2Commands::EnableClock | DisplayUpdate2Commands::EnableAnalog;
        }
        self.display(DisplayUpdate1Commands::Normal, mode, "fast");
    }

    /// Send control registers + MasterActivation for partial refresh, no busy wait.
    pub fn trigger_partial(&mut self) {
        let mut mode = DisplayUpdate2Commands::LoadLUT | DisplayUpdate2Commands::DisplayMode2 | DisplayUpdate2Commands::DisplayEnable;
        if !self.is_screen_on {
            self.is_screen_on = true;
            mode |= DisplayUpdate2Commands::EnableClock | DisplayUpdate2Commands::EnableAnalog;
        }
        self.display_update_ctrl1(DisplayUpdate1Commands::Normal);
        self.display_update_ctrl2(mode);
        self.send_command(SSD1677Command::MasterActivation);
    }

    pub fn write_ram(&mut self, color: Color, buffer: &Buffer) {
        let command = match color {
            Color::Red => SSD1677Command::WriteRamRed,
            Color::BlackWhite => SSD1677Command::WriteRamBw,
        };

        self.send_command(command);
        self.send_data(buffer);
        self.wait_while_busy("WriteRam");
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

    pub fn read_ram(&mut self, color: Color) {
        self.send_command(SSD1677Command::ReadRamOption);
        match color {
            Color::BlackWhite => self.send_byte(0x0),
            Color::Red => self.send_byte(0x1),
        }

        let mut buffer = ::core::cell::Cell::<[u8; (480*800/8) + 1]>::new([0u8; (480*800/8) + 1]);
        self.send_command(SSD1677Command::ReadRam);
        self.recv_data(&mut buffer);
        {
            print!("Returned data ({:?}): ", color);
            let data = buffer.as_array_of_cells();
            for i in 0..16 {
                print!("{:02X}", data[i].get());
            }
            println!("");
        }
    }

    pub fn set_data_mode(&mut self, x: DataEntryMode, y: DataEntryMode) {
        let x = if x == DataEntryMode::Increase { 0x1 } else { 0x0 };
        let y = if y == DataEntryMode::Increase { 0x2 } else { 0x0 };
        let val: u8 = x | y;

        self.send_command(SSD1677Command::DataEntryMode);
        self.send_byte(val);
    }

    fn send_command(&mut self, cmd: SSD1677Command) {
        self.transport.write_command(cmd as u8);
    }

    fn send_byte(&mut self, data: u8) {
        self.transport.write_data(&[data]);
    }

    fn send_data(&mut self, data: &Buffer) {
        // SAFETY: read-only, fixed lifetime use
        let data: &[u8] = unsafe { &*(data.as_ptr()) };

        self.transport.write_data(data);
    }

    fn recv_data(&mut self, data: &mut Buffer) {
        let buf: &mut [u8] = unsafe { &mut *(data.as_ptr()) };
        self.transport.read_data(buf);
    }
}
