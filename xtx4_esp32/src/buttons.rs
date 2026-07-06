use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation, AdcPin};
use esp_hal::gpio::{Pull, Input, AnyPin, InputConfig};
use esp_hal::analog::adc::AdcChannel;
use esp_hal::analog::adc::AdcCalBasic;
use esp_hal::analog::adc::AdcCalScheme;
use esp_hal::analog::adc::RegisterAccess;
use esp_hal::peripherals::{ADC1, GPIO1, GPIO2};
use esp_hal::Blocking;
use xtx4_platform_interface::{Buttons};

use crate::sleep::sleep_ms;

// ADC ranges for pin 1 (BACK, CONFIRM, LEFT, RIGHT)
// If ADC value is between range[i+1] and range[i], button i is pressed
const ADC_NO_BUTTON: u16 = 3800;
const ADC_RANGES_1: [u16; 5] = [ADC_NO_BUTTON, 3100, 2090, 750, 0];

// ADC ranges for pin 2 (UP, DOWN)
const ADC_RANGES_2: [u16; 3] = [ADC_NO_BUTTON, 1120, 0];


fn read_adc_button<'a, ADC, Pin, Calibration>(adc: &mut Adc<'a, ADC, Blocking>, pin: &mut AdcPin<Pin, ADC, Calibration>, ranges: &[u16]) -> Option<usize>
where
    ADC: RegisterAccess + 'a,
    Pin: AdcChannel,
    Calibration: AdcCalScheme<ADC>
{
    let value: u16 = nb::block!(adc.read_oneshot(pin)).unwrap();
    for i in 0..ranges.len() - 1 {
        if ranges[i] >= value && value > ranges[i+1] {
            return Some(i);
        }
    }
    None
}

//pub struct Xtx4Buttons<ADC, FacePin, SidePin>
//where
//    ADC: RegisterAccess + Peripheral<P = ADC> + 'static,
//    FacePin: AdcChannel + AnalogPin,
//    SidePin: AdcChannel + AnalogPin,
//    <ADC as esp_hal::peripheral::Peripheral>::P: esp_hal::analog::adc::RegisterAccess,
//{
//    adc: Adc<'static, ADC>,
//    face_pin: AdcPin<FacePin, ADC>,
//    side_pin: AdcPin<SidePin, ADC>,
//    power: Input<'static>,
//}

pub struct Xtx4Buttons
{
    adc: Adc<'static, ADC1<'static>, Blocking>,
    face_pin: AdcPin<GPIO1<'static>, ADC1<'static>, AdcCalBasic<ADC1<'static>>>,
    side_pin: AdcPin<GPIO2<'static>, ADC1<'static>, AdcCalBasic<ADC1<'static>>>,
    power: Input<'static>,
}

impl Xtx4Buttons
{
    pub fn new(adc: ADC1<'static>, face_pin: GPIO1<'static>, side_pin: GPIO2<'static>, power: AnyPin<'static>) -> Self {
        let mut adc_config = AdcConfig::new();
        //let face_pin = adc_config.enable_pin(face_pin, Attenuation::_11dB);
        let face_pin = adc_config.enable_pin_with_cal::<_, AdcCalBasic<ADC1<'static>>>(face_pin, Attenuation::_11dB);
        //let side_pin = adc_config.enable_pin(side_pin, Attenuation::_11dB);
        let side_pin = adc_config.enable_pin_with_cal::<_, AdcCalBasic<ADC1<'static>>>(side_pin, Attenuation::_11dB);
        let adc = Adc::new(adc, adc_config);

        let power_config = InputConfig::default().with_pull(Pull::Up);
        let power = Input::new(power, power_config);

        Self {
            adc,
            face_pin,
            side_pin,
            power,
        }
    }

    fn scan_buttons(&mut self) -> Buttons {
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

        let power_val = self.power.is_low();
        if power_val {
            state |= Buttons::POWER;
        }

        state
    }

    // Debounce in a single read because we have
    // lots of time. We can do this more efficiently
    // if it becomes a problem.
    pub fn button_state(&mut self) -> Buttons {
        const DEBOUNCE_MS: u32 = 5;

        let first = self.scan_buttons();
        sleep_ms(DEBOUNCE_MS);
        let second = self.scan_buttons();
        first & second
    }
}

impl ssd1677::ButtonReader for Xtx4Buttons {
    fn button_state(&mut self) -> Buttons {
        self.button_state()
    }
}
