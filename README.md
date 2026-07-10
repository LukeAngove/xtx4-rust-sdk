# xtx4-rust-sdk

Rust SDK and emulator for the Xteink X4 e-paper device (ESP32-C3).

## Crates

### Core traits
| Crate | Description |
|-------|-------------|
| `xtx4-platform-interface` | Platform trait, `Buttons`, `Framebuffer`, `Rectangle` types |
| `xtx4-display` | `DisplayController` trait + `Display` wrapper for non-blocking refresh |
| `ssd1677` | SSD1677 e-paper driver chip (controller + register definitions) |
| `xtx4-buttons` | `ButtonReader` trait |
| `xtx4-host` | `Host` struct + `now_ms()`/`delay_ms()` |

### Hardware backends (ESP32-C3)
| Crate | Description |
|-------|-------------|
| `xtx4-esp32` | ESP32 platform implementation |
| `ssd1677-esp` | SPI/GPIO transport for SSD1677 |
| `xtx4-buttons-adc` | ADC resistor-ladder button reader |
| `xtx4-host-esp` | RTC sleep, CPU frequency scaling |

### Emulation / testing backends (x86_64)
| Crate | Description |
|-------|-------------|
| `ssd1677-pbm` | PBM frame capture for regression testing |
| `ssd1677-minifb` | Minifb window display emulation |
| `xtx4-buttons-stdin` | Terminal keyboard input |
| `xtx4-buttons-mock` | Scripted button sequence for tests |
| `xtx4-buttons-minifb` | Minifb window key input |
| `xtx4-host-emulated` | Emulated host (now_ms, sleep, exit on deep_sleep) |
| `xtx4-desktop` | Desktop framebuffer platform |

### Application
| Crate | Description |
|-------|-------------|
| `xtx4-platform` | High-level `XtX4` struct, canvas rendering, input state manager |
| `xtx4-sample` | Sample app demonstrating SDK features |

## Quick Start

```bash
# Build for ESP32-C3
cargo build-esp

# Flash and monitor
cargo run-esp

# Regression tests (x86_64)
cargo test-regression

# Desktop build
cargo run-desktop
```

## License

GPL-3.0-or-later
