// Raw SPI/GPIO abstraction for the SSD1677 driver.
// Real hardware and emulated mock both implement this trait.

use xtx4_platform_interface::Buttons;

/// Read button state from platform-specific hardware (ADC or stdin).
pub trait ButtonReader {
    fn button_state(&mut self) -> Buttons;
}

/// Low-level transport operations needed by the SSD1677 driver.
pub trait DisplayInterface {
    /// Set D/C low, CS low, write one command byte, CS high.
    fn write_command(&mut self, cmd: u8);
    /// Set D/C high, CS low, write data bytes, CS high.
    fn write_data(&mut self, data: &[u8]);
    /// Set D/C high, CS low, read bytes into buffer (while writing dummy 0x00), CS high.
    fn read_data(&mut self, data: &mut [u8]);
    /// Toggle RST low for >=2ms then high (hardware reset).
    fn reset(&mut self);
    /// Return true if BUSY pin is high (controller busy).
    fn busy_high(&self) -> bool;
}