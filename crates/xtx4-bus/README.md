# xtx4-bus

Shared SPI bus singleton for the Xteink X4.

The ESP32-C3 has one usable SPI peripheral (SPI2) shared between the SSD1677
display and the SD card. This crate holds a `static Mutex<RefCell<MaybeUninit<Spi>>>`
initialized once at boot and accessed by both drivers.

```rust
// Platform init (xtx4-esp32)
let spi = Spi::new(peripherals.SPI2, config).with_sck(...)...;
xtx4_bus::init(spi);

// Display (ssd1677-esp)
let dev = CriticalSectionDevice::new(xtx4_bus::get(), cs, delay);

// SD card (sd-storage)
xtx4_bus::with(|spi| { spi.write(data).unwrap(); });
```
