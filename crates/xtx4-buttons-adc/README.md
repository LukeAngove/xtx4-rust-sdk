# xtx4-buttons-adc

Hardware button reader for the Xteink X4: ADC resistor ladder on GPIO1+GPIO2
(face + side buttons) with power button on GPIO3. Maps voltage thresholds to
`Buttons` bitflags.

## Types

- **`ButtonsAdc`** — Synchronous polling. Reads ADC on every `button_state()`
  call with a blocking 5ms debounce delay.
- **`ButtonsAdcIntr`** — Interrupt-driven. A 10ms SYSTIMER alarm reads the ADC
  in the ISR, debounces in hardware (20ms), and caches the result.
  `button_state()` returns the cached value instantly with zero blocking. Also
  tracks press and release latches so short taps during display updates
  are not missed.

Both implement the `ButtonReader` trait from `xtx4-buttons`.
