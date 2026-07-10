#![no_std]

use esp_hal::peripherals::LPWR;
use esp_hal::rtc_cntl::sleep::{RtcioWakeupSource, WakeupLevel};
use esp_hal::rtc_cntl::Rtc;
use esp_hal::gpio::{AnyPin, RtcPinWithResistors};

pub fn now_ms() -> u32 {
    esp_hal::time::Instant::now().duration_since_epoch().as_millis() as u32
}

pub fn delay_ms(ms: u32) {
    esp_hal::delay::Delay::new().delay_millis(ms);
}

pub struct Host {
    rtc: Rtc<'static>,
    wake_pin_num: u8,
}

impl Host {
    pub fn new(lpwr: LPWR<'static>, wake_pin_num: u8) -> Self {
        let rtc = Rtc::new(lpwr);
        Self { rtc, wake_pin_num }
    }

    /// Enter deep sleep. MCU powers off; wakes on cold boot from power button.
    pub fn deep_sleep(&mut self) -> ! {
        let mut pin = unsafe { AnyPin::steal(self.wake_pin_num) };
        let mut pair: (&mut dyn RtcPinWithResistors, WakeupLevel) = (&mut pin, WakeupLevel::Low);
        let wake = RtcioWakeupSource::new(core::slice::from_mut(&mut pair));
        let mut config = esp_hal::rtc_cntl::sleep::RtcSleepConfig::deep();
        config.set_deep_slp_reject(false);
        self.rtc.sleep(&config, &[&wake]);
        unreachable!();
    }

    /// Enter light sleep. CPU pauses, RAM preserved, resumes when button pressed.
    pub fn light_sleep(&mut self) {
        let mut pin = unsafe { AnyPin::steal(self.wake_pin_num) };
        let mut pair: (&mut dyn RtcPinWithResistors, WakeupLevel) = (&mut pin, WakeupLevel::Low);
        let wake = RtcioWakeupSource::new(core::slice::from_mut(&mut pair));
        self.rtc.sleep_light(&[&wake]);
    }

    /// Lower CPU frequency to save power while staying active.
    /// 10MHz via XTAL/4 when enabled, 80MHz PLL when disabled.
    /// Matches IDF: rtc_clk_cpu_freq_to_xtal(10,4) / rtc_clk_cpu_freq_to_pll_mhz(80)
    pub fn set_low_power(&mut self, enabled: bool) {
        let sys = esp_hal::peripherals::SYSTEM::regs();
        if enabled {
            esp_hal::rom::ets_update_cpu_frequency_rom(10);
            sys.sysclk_conf().modify(|_, w| unsafe { w.pre_div_cnt().bits(3) });
            sys.sysclk_conf().modify(|_, w| unsafe { w.soc_clk_sel().bits(0) });
        } else {
            sys.sysclk_conf().modify(|_, w| unsafe { w.pre_div_cnt().bits(0) });
            sys.sysclk_conf().modify(|_, w| unsafe { w.soc_clk_sel().bits(1) });
            sys.cpu_per_conf().modify(|_, w| unsafe { w.cpuperiod_sel().bits(0) });
            esp_hal::rom::ets_update_cpu_frequency_rom(80);
        }
    }
}
