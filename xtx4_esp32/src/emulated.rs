use std::io::Read;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::time::Duration;

use xtx4_platform_interface::Buttons;
use crate::display_transport::ButtonReader;

// ── Raw terminal via stty ────────────────────────────────────────────

struct RawMode;
impl RawMode {
    fn enable() -> Self {
        std::process::Command::new("stty")
            .args(["-icanon", "-echo"])
            .status().ok();
        RawMode
    }
}
impl Drop for RawMode {
    fn drop(&mut self) {
        std::process::Command::new("stty")
            .args(["sane"])
            .status().ok();
    }
}

// ── EmulatedButtons: mirrors Xtx4Buttons but reads from stdin ────────

static LAST_KEY: AtomicU8 = AtomicU8::new(0);

pub struct EmulatedButtons {
    _reader_thread: thread::JoinHandle<()>,
    _raw_guard: RawMode,
}

impl EmulatedButtons {
    pub fn new() -> Self {
        let _raw_guard = RawMode::enable();

        let handle = thread::spawn(|| {
            let mut buf = [0u8; 1];
            loop {
                match std::io::stdin().read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        LAST_KEY.store(buf[0], Ordering::Relaxed);
                        if buf[0] == b'q' || buf[0] == b'Q' { break; }
                    }
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Self { _reader_thread: handle, _raw_guard }
    }
}

impl ButtonReader for EmulatedButtons {
    fn button_state(&mut self) -> Buttons {
        let key = LAST_KEY.swap(0, Ordering::Relaxed);
        if key == b'q' || key == b'Q' {
            std::process::exit(0);
        }
        match key {
            b'd' | b'D' | b'r' | b'R' => Buttons::LEFT_OUTER,
            b'f' | b'F' => Buttons::LEFT_INNER,
            b'j' | b'J' => Buttons::RIGHT_INNER,
            b'k' | b'K' => Buttons::RIGHT_OUTER,
            b'l' | b'L' => Buttons::SIDE_TOP,
            b';' => Buttons::SIDE_BOTTOM,
            b'p' | b'P' => Buttons::POWER,
            _ => Buttons::empty(),
        }
    }
}