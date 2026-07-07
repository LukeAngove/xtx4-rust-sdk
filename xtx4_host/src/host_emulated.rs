pub fn now_ms() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u32
}

pub fn delay_ms(ms: u32) {
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
}