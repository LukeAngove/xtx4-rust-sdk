// ISR-driven button reader.  See module-level docs in lib.rs.
//
// Architecture:
//   ButtonsAdc           – lives in INTR_BUTTONS static, ISR calls scan_buttons()
//   PeriodicTimer        – lives in ISR_TIMER static, ISR clears interrupt
//   CURRENT              – debounced state (ISR writes, main reads)
//   PRESSED_SINCE_POLL   – rising-edge latch (ISR sets, main reads + clears)
//   RELEASED_SINCE_POLL  – falling-edge latch (ISR sets, main reads + clears)
//   button_state()       – returns CURRENT | PRESSED_SINCE_POLL | RELEASED_SINCE_POLL,
//                           clears latches

use core::{
    cell::RefCell,
    mem::MaybeUninit,
};

use critical_section::Mutex;

use esp_hal::{
    gpio::Input,
    interrupt::{self, InterruptHandler, Priority},
    peripherals::{ADC1, GPIO1, GPIO2, SYSTIMER, Interrupt},
    timer::{
        systimer::{Alarm, SystemTimer},
        PeriodicTimer,
    },
    Blocking,
};
use xtx4_platform_interface::Buttons;

use xtx4_buttons::ButtonReader;

use super::polling::ButtonsAdc;

// --- ISR globals -------------------------------------------------------
//
// SAFETY: all are single-writer (ISR) / single-reader (main). Plain
// byte access is naturally atomic on RV32. Rust requires `unsafe` for
// `static mut`, but the access pattern is sound on single-core.

static INTR_BUTTONS: Mutex<RefCell<MaybeUninit<ButtonsAdc>>> =
    Mutex::new(RefCell::new(MaybeUninit::uninit()));

static mut CURRENT: u8 = 0;
static mut PRESSED_SINCE_POLL: u8 = 0;
static mut RELEASED_SINCE_POLL: u8 = 0;
static mut DB_COUNT: u8 = 0;
static mut LAST_RAW: u8 = 0;

/// PeriodicTimer, accessed only by the ISR.
static ISR_TIMER: Mutex<RefCell<MaybeUninit<PeriodicTimer<'static, Blocking>>>> =
    Mutex::new(RefCell::new(MaybeUninit::uninit()));

// --- ISR ----------------------------------------------------------------

extern "C" fn timer_isr() {
    critical_section::with(|cs| {
        // Acknowledge the interrupt immediately.
        // SAFETY: ISR_TIMER is initialised before interrupts fire.
        let mut timer_ref = ISR_TIMER.borrow_ref_mut(cs);
        let timer = unsafe { timer_ref.assume_init_mut() };
        timer.clear_interrupt();
        drop(timer_ref);

        let mut ref_mut = INTR_BUTTONS.borrow_ref_mut(cs);
        // SAFETY: initialised once in new() before interrupts are enabled.
        let buttons = unsafe { ref_mut.assume_init_mut() };

        let raw = buttons.scan_buttons().bits();
        // SAFETY: single writer (ISR), single reader (main thread).
        let prev = unsafe { LAST_RAW };

        if raw == prev {
            let c = unsafe { DB_COUNT }.saturating_add(1);
            unsafe { DB_COUNT = c; }
            if c >= 2 {
                let old = unsafe { CURRENT };
                unsafe { CURRENT = raw; }

                // Rising edges (press), falling edges (release).
                let rising = raw & !old;
                let falling = old & !raw;
                unsafe {
                    PRESSED_SINCE_POLL |= rising;
                    RELEASED_SINCE_POLL |= falling;
                }
            }
        } else {
            unsafe { DB_COUNT = 0; }
        }
        unsafe { LAST_RAW = raw; }
    });
}

// --- Public type --------------------------------------------------------

/// Interrupt-driven button reader.
///
/// A 10 ms SYSTIMER alarm fires the ISR which reads both ADC pins,
/// debounces (20 ms), and caches the result.  The ISR also tracks
/// rising and falling edges so that short taps during display
/// updates are not lost.
///
/// `button_state()` returns the cached value with zero blocking.
pub struct ButtonsAdcIntr;

impl ButtonsAdcIntr {
    /// Create a new interrupt-driven reader.
    ///
    /// Takes the same arguments as [`ButtonsAdc::new`] plus
    /// `systimer_periph` — the SYSTIMER peripheral.  A 10 ms
    /// periodic alarm is configured internally; the ISR clears
    /// its own interrupt flag.
    pub fn new(
        adc: ADC1<'static>,
        face_pin: GPIO1<'static>,
        side_pin: GPIO2<'static>,
        power: Input<'static>,
        systimer_periph: SYSTIMER<'static>,
    ) -> Self {
        // Stash the poller in the static *before* enabling interrupts.
        critical_section::with(|cs| {
            INTR_BUTTONS
                .borrow_ref_mut(cs)
                .write(ButtonsAdc::new(adc, face_pin, side_pin, power));
        });

        // Timer: 10 ms periodic.
        let systimer = SystemTimer::new(systimer_periph);
        let alarm = systimer.alarm0;
        // SAFETY: Systimer is a permanent peripheral.
        let alarm: Alarm<'static> = unsafe { core::mem::transmute(alarm) };
        let mut timer = PeriodicTimer::new(alarm);
        timer.clear_interrupt();
        timer.set_interrupt_handler(InterruptHandler::new(timer_isr, Priority::min()));
        timer.listen();

        // Stash in static before start — ensures ISR sees initialised timer.
        critical_section::with(|cs| {
            ISR_TIMER.borrow_ref_mut(cs).write(timer);
        });

        // Start through the static so the ISR can clear interrupts.
        critical_section::with(|cs| {
            let mut timer_ref = ISR_TIMER.borrow_ref_mut(cs);
            let timer = unsafe { timer_ref.assume_init_mut() };
            interrupt::enable(Interrupt::SYSTIMER_TARGET0, Priority::min());
            timer
                .start(esp_hal::time::Duration::from_millis(10))
                .expect("timer start");
        });

        ButtonsAdcIntr
    }
}

impl ButtonReader for ButtonsAdcIntr {
    fn button_state(&mut self) -> Buttons {
        // SAFETY: single reader (main thread), ISR is single writer.
        let current = unsafe { CURRENT };
        let pressed = unsafe { PRESSED_SINCE_POLL };
        let released = unsafe { RELEASED_SINCE_POLL };
        unsafe {
            PRESSED_SINCE_POLL = 0;
            RELEASED_SINCE_POLL = 0;
        }

        Buttons::from_bits(current | pressed | released).unwrap_or(Buttons::empty())
    }
}
