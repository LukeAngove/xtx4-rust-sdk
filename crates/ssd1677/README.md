# ssd1677

SSD1677 e-paper display driver chip.

- `DisplayInterface` trait — SPI/GPIO transport abstraction
- `SSD1677<T: DisplayInterface>` — register-level chip control
- `Ssd1677Controller<T: DisplayInterface>` — implements `DisplayController` with rotation, partial/full refresh
- LUT and command definitions
