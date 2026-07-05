// ESP32-C3 hardware transport implementation.
// Wraps the real SPI, CS, DC, RST, BUSY pins.

use esp_hal::{
    time::Rate,
    spi::{
        Mode,
        master::{Spi, Config},
    },
    gpio::{Level, Input, Output, AnyPin},
};
use esp_hal::gpio::InputConfig;
use esp_hal::gpio::OutputConfig;

use crate::sleep::sleep_ms;
use crate::display_transport::DisplayTransport;

pub struct EspTransport {
    spi:  Spi<'static, esp_hal::Blocking>,
    cs:   Output<'static>,
    dc:   Output<'static>,
    rst:  Output<'static>,
    busy: Input<'static>,
}

pub struct EspTransportBuilder {
    pub spi:   AnyPin<'static>,
    pub sck:   AnyPin<'static>,
    pub mosi:  AnyPin<'static>,
    pub miso:  AnyPin<'static>,
    pub cs:    AnyPin<'static>,
    pub dc:    AnyPin<'static>,
    pub rst:   AnyPin<'static>,
    pub busy:  AnyPin<'static>,
}

impl EspTransport {
    pub fn new(b: EspTransportBuilder) -> Self {
        let config = Config::default()
            .with_frequency(Rate::from_mhz(40u32))
            .with_mode(Mode::_0);

        let spi = Spi::new(b.spi, config)
            .expect("SPI failed to initialise")
            .with_sck(b.sck)
            .with_mosi(b.mosi)
            .with_miso(b.miso);

        let out_cfg = OutputConfig::default();
        let cs   = Output::new(b.cs, Level::High, out_cfg);
        let dc   = Output::new(b.dc, Level::High, out_cfg);
        let rst  = Output::new(b.rst, Level::High, out_cfg);

        let busy_cfg = InputConfig::default().with_pull(esp_hal::gpio::Pull::None);
        let busy = Input::new(b.busy, busy_cfg);

        Self { spi, cs, dc, rst, busy }
    }
}

impl DisplayTransport for EspTransport {
    fn write_command(&mut self, cmd: u8) {
        self.dc.set_low();
        self.cs.set_low();
        self.spi.write(&[cmd]).unwrap();
        self.cs.set_high();
    }

    fn write_data(&mut self, data: &[u8]) {
        self.dc.set_high();
        self.cs.set_low();
        if !data.is_empty() {
            self.spi.write(data).unwrap();
        }
        self.cs.set_high();
    }

    fn read_data(&mut self, data: &mut [u8]) {
        self.dc.set_high();
        self.cs.set_low();
        if !data.is_empty() {
            self.spi.read(data);
        }
        self.cs.set_high();
    }

    fn reset(&mut self) {
        self.rst.set_high();
        sleep_ms(20);
        self.rst.set_low();
        sleep_ms(2);
        self.rst.set_high();
        sleep_ms(20);
    }

    fn busy_high(&self) -> bool {
        self.busy.is_high()
    }

    fn delay_ms(&mut self, ms: u32) {
        sleep_ms(ms);
    }

    fn millis(&self) -> u32 {
        esp_hal::time::Instant::now().duration_since_epoch().as_millis() as u32
    }
}
