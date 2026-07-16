# xtx4-storage

Storage abstraction: SD card on ESP32, host folder on desktop.

| Platform | Backend | Root |
|----------|---------|------|
| ESP32 (riscv32) | `embedded-sdmmc` over SPI, GPIO12 CS | SD card root |
| Desktop (x86_64) | `std::fs` | `./sd_root/` |

```rust
let mut storage = Storage::new();  // or Storage::new(pin) on ESP32

storage.write_file("/test.txt", b"hello")?;
let mut buf = [0u8; 128];
let n = storage.read_file("/test.txt", &mut buf)?;
storage.list_dir("/", &mut |name| { println!("{name}"); true })?;
storage.exists("/test.txt");
```
