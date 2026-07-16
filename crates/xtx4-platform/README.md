# xtx4-platform

High-level application API: `XtX4` struct, `Canvas` rendering, `InputState`.

```rust
let mut device = XtX4::new();

// Draw
let mut canvas = device.canvas();
Text::new("Hello", Point::new(10, 20), style).draw(&mut canvas)?;
device.display_flush();

// Input
let input = device.update_input();
if input.was_pressed(Button::LeftOuter) { ... }

// Storage
device.storage().write_file("/test.txt", b"data")?;
```
