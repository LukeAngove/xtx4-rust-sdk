# xtx4-esp32

ESP32-C3 platform implementation for the Xteink X4.

Wires together the display (SSD1677), buttons (ADC), host (RTC/power), and
storage (SD card). Creates the SPI bus and initializes the shared `xtx4_bus`
singleton.

## Pinout

| Signal | GPIO |
|--------|------|
| SCLK   | 8    |
| MOSI   | 10   |
| MISO   | 7    |
| EPD CS | 21   |
| EPD DC | 4    |
| EPD RST| 5    |
| EPD BUSY| 6   |
| SD CS  | 12   |
| ADC btns| 1, 2 |
| Power  | 3    |
