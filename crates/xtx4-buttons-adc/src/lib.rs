#![cfg_attr(target_arch = "riscv32", no_std)]

// ESP32-C3 ADC button reader.
//
// Two types:
//   ButtonsAdc     – synchronous: reads ADC on every poll, 5ms blocking debounce.
//   ButtonsAdcIntr – ISR-driven: 10ms SYSTIMER alarm reads ADC in the ISR,
//                     debounces, and tracks transitions so short presses
//                     during display updates are not missed.

// ════════════════════════════════════════════════════════════════════════
//  RISCV  –  real hardware
// ════════════════════════════════════════════════════════════════════════

#[cfg(target_arch = "riscv32")]
mod polling;
#[cfg(target_arch = "riscv32")]
mod intr;

#[cfg(target_arch = "riscv32")]
pub use polling::ButtonsAdc;
#[cfg(target_arch = "riscv32")]
pub use intr::ButtonsAdcIntr;

// ════════════════════════════════════════════════════════════════════════
//  Tests  –  simulated ISR / poll loop, no hardware
// ════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use xtx4_platform_interface::Buttons;

    /// A software copy of the ISR state machine, used to validate
    /// behaviour without hardware.
    struct State {
        current: u8,
        pressed_since_poll: u8,
        released_since_poll: u8,
        db_count: u8,
        last_raw: u8,
    }

    impl State {
        fn new() -> Self {
            Self { current: 0, pressed_since_poll: 0, released_since_poll: 0, db_count: 0, last_raw: 0 }
        }

        fn isr_tick(&mut self, raw: u8) {
            if raw == self.last_raw {
                self.db_count = self.db_count.saturating_add(1);
                if self.db_count >= 2 {
                    let old = self.current;
                    self.current = raw;
                    self.pressed_since_poll |= raw & !old;
                    self.released_since_poll |= old & !raw;
                }
            } else {
                self.db_count = 0;
            }
            self.last_raw = raw;
        }

        fn poll(&mut self) -> u8 {
            let r = self.current | self.pressed_since_poll | self.released_since_poll;
            self.pressed_since_poll = 0;
            self.released_since_poll = 0;
            r
        }
    }

    #[test]
    fn steady_idle() {
        let mut s = State::new();
        for _ in 0..10 { s.isr_tick(0); }
        assert_eq!(s.poll(), 0);
        assert_eq!(s.poll(), 0);
    }

    #[test]
    fn hold_across_polls() {
        let mut s = State::new();
        for _ in 0..5 { s.isr_tick(Buttons::LEFT_OUTER.bits()); }
        assert_eq!(s.poll(), Buttons::LEFT_OUTER.bits());
        assert_eq!(s.poll(), Buttons::LEFT_OUTER.bits());
        assert_eq!(s.poll(), Buttons::LEFT_OUTER.bits());
    }

    #[test]
    fn tap_between_polls() {
        let mut s = State::new();
        for _ in 0..5 { s.isr_tick(Buttons::LEFT_OUTER.bits()); }
        for _ in 0..5 { s.isr_tick(0); }
        assert_ne!(s.poll() & Buttons::LEFT_OUTER.bits(), 0);
        assert_eq!(s.poll(), 0);
    }

    #[test]
    fn two_quick_taps_between_polls() {
        let mut s = State::new();
        let b = Buttons::LEFT_OUTER.bits();
        for _ in 0..3 { s.isr_tick(b); }
        for _ in 0..3 { s.isr_tick(0); }
        for _ in 0..3 { s.isr_tick(b); }
        for _ in 0..3 { s.isr_tick(0); }
        assert_ne!(s.poll() & b, 0);
        assert_eq!(s.poll(), 0);
    }

    #[test]
    fn press_then_release_between_polls() {
        let mut s = State::new();
        let b = Buttons::LEFT_OUTER.bits();
        for _ in 0..3 { s.isr_tick(b); }
        for _ in 0..3 { s.isr_tick(0); }
        assert!(s.poll() & b != 0);
        assert_eq!(s.poll(), 0);
    }

    #[test]
    fn power_button_combined() {
        let mut s = State::new();
        let face = Buttons::RIGHT_INNER.bits();
        let pwr  = Buttons::POWER.bits();
        for _ in 0..3 { s.isr_tick(face | pwr); }
        assert!(s.poll() & face != 0);
    }

    #[test]
    fn debounce_noise_rejection() {
        let mut s = State::new();
        let b = Buttons::LEFT_OUTER.bits();
        for i in 0..20 {
            s.isr_tick(if i % 2 == 0 { b } else { 0 });
        }
        assert_eq!(s.poll(), 0);
    }

    #[test]
    fn release_latch_detected() {
        // Button held, then released mid-busy-period.
        let mut s = State::new();
        let b = Buttons::LEFT_OUTER.bits();

        // Hold for several ticks.
        for _ in 0..5 { s.isr_tick(b); }
        // Poll: pressed event visible.
        assert!(s.poll() & b != 0);
        // Now release.
        for _ in 0..5 { s.isr_tick(0); }
        // Poll: release event visible.
        assert!(s.poll() & b != 0);
        // Next poll: nothing.
        assert_eq!(s.poll(), 0);
    }

    #[test]
    fn press_and_release_same_frame() {
        // Tap entirely between polls.
        let mut s = State::new();
        let b = Buttons::LEFT_OUTER.bits();

        for _ in 0..3 { s.isr_tick(b); }
        for _ in 0..3 { s.isr_tick(0); }

        // First poll: both press and release in one frame.
        let r = s.poll();
        assert!(r & b != 0);
        // Next poll: nothing.
        assert_eq!(s.poll(), 0);
    }
}
