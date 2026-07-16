# xtx4-display

`DisplayController` trait and `Display` wrapper.

- `DisplayController` — low-level trait: start_update, finish_update, sleep, wake
- `Display<D: DisplayController>` — high-level wrapper with blocking helpers
  (flush_full, flush_partial, fast_full)
