pub fn now_ms() -> u32 {
    esp_hal::time::Instant::now().duration_since_epoch().as_millis() as u32
}

pub fn delay_ms(ms: u32) {
    esp_hal::delay::Delay::new().delay_millis(ms);
}