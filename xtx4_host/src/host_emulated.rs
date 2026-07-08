pub fn now_ms() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u32
}

pub fn delay_ms(ms: u32) {
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
}

pub struct Host;

impl Host {
    pub fn new() -> Self {
        Host
    }

    pub fn deep_sleep(&mut self) -> ! {
        std::process::exit(0);
    }

    pub fn light_sleep(&mut self) {
        // no-op on x86_64
    }

    pub fn set_low_power(&mut self, _enabled: bool) {
        // no-op on x86_64
    }
}
