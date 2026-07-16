# ssd1677-esp

ESP32-C3 hardware transport for the SSD1677 display.

`EspInterface` uses `CriticalSectionDevice` from `embedded-hal-bus` backed by
the shared `xtx4_bus` singleton. CS (GPIO21) is auto-managed; DC, RST, BUSY
are handled manually.

```rust
let dev = EspInterface::new(EspInterfaceBuilder {
    cs:   peripherals.GPIO21.into(),
    dc:   peripherals.GPIO4.into(),
    rst:  peripherals.GPIO5.into(),
    busy: peripherals.GPIO6.into(),
});
```
