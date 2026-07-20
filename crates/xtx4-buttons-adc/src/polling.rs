// Synchronous polling button reader.  See module-level docs in lib.rs.

use esp_hal::{
    analog::adc::{
        Adc, AdcCalBasic, AdcCalScheme, AdcChannel, AdcConfig, AdcPin, Attenuation,
        RegisterAccess,
    },
    gpio::Input,
    peripherals::{ADC1, GPIO1, GPIO2},
    Blocking,
};
use xtx4_platform_interface::Buttons;

use xtx4_buttons::ButtonReader;

// ── ADC ranges ────────────────────────────────────────────────────────

const ADC_NO_BUTTON: u16 = 3800;
const ADC_RANGES_1: [u16; 5] = [ADC_NO_BUTTON, 3100, 2090, 750, 0];
const ADC_RANGES_2: [u16; 3] = [ADC_NO_BUTTON, 1120, 0];

// ── Helpers ───────────────────────────────────────────────────────────

fn read_adc_button<'a, ADC, Pin, Cal>(
    adc: &mut Adc<'a, ADC, Blocking>,
    pin: &mut AdcPin<Pin, ADC, Cal>,
    ranges: &[u16],
) -> Option<usize>
where
    ADC: RegisterAccess + 'a,
    Pin: AdcChannel,
    Cal: AdcCalScheme<ADC>,
{
    let value: u16 = nb::block!(adc.read_oneshot(pin)).unwrap();
    for i in 0..ranges.len() - 1 {
        if ranges[i] >= value && value > ranges[i + 1] {
            return Some(i);
        }
    }
    None
}

// ── Public type ────────────────────────────────────────────────────────

/// Synchronous (polling) button reader.
///
/// Reads ADC pins on every `button_state()` call with a 5 ms
/// blocking debounce delay.  Suitable for simple event loops where
/// the occasional blocking read is acceptable.
pub struct ButtonsAdc {
    adc: Adc<'static, ADC1<'static>, Blocking>,
    face_pin: AdcPin<GPIO1<'static>, ADC1<'static>, AdcCalBasic<ADC1<'static>>>,
    side_pin: AdcPin<GPIO2<'static>, ADC1<'static>, AdcCalBasic<ADC1<'static>>>,
    power: Input<'static>,
}

impl ButtonsAdc {
    /// Create a new polling reader.
    ///
    /// `adc` — ADC1 peripheral.
    /// `face_pin` — GPIO1 (face buttons: LEFT/RIGHT, outer/inner).
    /// `side_pin` — GPIO2 (side buttons: TOP/BOTTOM).
    /// `power`    — GPIO3 configured as digital input with pull-up.
    pub fn new(
        adc: ADC1<'static>,
        face_pin: GPIO1<'static>,
        side_pin: GPIO2<'static>,
        power: Input<'static>,
    ) -> Self {
        let mut adc_config = AdcConfig::new();
        let face_pin = adc_config.enable_pin_with_cal::<_, AdcCalBasic<ADC1<'static>>>(
            face_pin,
            Attenuation::_11dB,
        );
        let side_pin = adc_config.enable_pin_with_cal::<_, AdcCalBasic<ADC1<'static>>>(
            side_pin,
            Attenuation::_11dB,
        );
        let adc = Adc::new(adc, adc_config);

        Self {
            adc,
            face_pin,
            side_pin,
            power,
        }
    }

    pub(crate) fn scan_buttons(&mut self) -> Buttons {
        let mut state = Buttons::empty();

        if let Some(btn) = read_adc_button(&mut self.adc, &mut self.face_pin, &ADC_RANGES_1) {
            state |= match btn {
                0 => Buttons::LEFT_OUTER,
                1 => Buttons::LEFT_INNER,
                2 => Buttons::RIGHT_INNER,
                3 => Buttons::RIGHT_OUTER,
                _ => Buttons::empty(),
            };
        }

        if let Some(btn) = read_adc_button(&mut self.adc, &mut self.side_pin, &ADC_RANGES_2) {
            state |= match btn {
                0 => Buttons::SIDE_TOP,
                1 => Buttons::SIDE_BOTTOM,
                _ => Buttons::empty(),
            };
        }

        if self.power.is_low() {
            state |= Buttons::POWER;
        }

        state
    }
}

impl ButtonReader for ButtonsAdc {
    fn button_state(&mut self) -> Buttons {
        const DEBOUNCE_MS: u32 = 5;

        let first = self.scan_buttons();
        xtx4_host::delay_ms(DEBOUNCE_MS);
        let second = self.scan_buttons();
        first & second
    }
}
