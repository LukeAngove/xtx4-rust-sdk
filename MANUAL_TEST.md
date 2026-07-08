# Hardware Test Procedure — Power Management

Run `cargo run-esp`, observe the display and serial output.

## Phase 1: Normal Button Operation

| Step | Button | Expected Result |
|------|--------|-----------------|
| 1 | (boot) | "Blue, Purple! asdf!" text appears, then "Hi!" in top-left corner |
| 2 | LEFT_OUTER | Three black squares appear (fast refresh) |
| 3 | LEFT_INNER | Screen flashes black→white, clears to white |
| 4 | RIGHT_INNER | Small black square appears |
| 5 | RIGHT_OUTER | Small white square appears (slightly offset, partial clear) |
| 6 | SIDE_TOP | Five pairs of black/white columns appear rapidly (ghosting) |
| 7 | SIDE_BOTTOM | Five more overlapping columns appear |

## Phase 2: POWER + Button Combos

| Step | Combo | Expected Result |
|------|-------|-----------------|
| 8 | Hold POWER + LEFT_OUTER, release | Low power ON. |
| 9 | LEFT_INNER (no POWER) | No change in display — display is asleep, flush skipped automatically. |
| 10 | Hold POWER + LEFT_INNER, release | Display wakes. Screen preserved from before 8. Low power OFF. |
| 11 | LEFT_INNER (no POWER) | Screen flashes white. Display fully awake. |
| 12 | Hold POWER + RIGHT_INNER, release | Display goes dark. CPU enters light sleep. |
| 13 | LEFT_INNER (no POWER) | Nothing — CPU is paused, button ignored. |
| 14 | Press POWER briefly | Display wakes. White screen. Light sleep exit. |
| 15 | LEFT_OUTER (no POWER) | Three squares appear. System fully operational. |
| 16 | Hold POWER + RIGHT_OUTER, release | Full power-off. Display dark. No serial output. |
| 17 | Press POWER | Device cold boots. "Blue, Purple! asdf!" and "Hi!" appear. |

## Pass Criteria

- Every "Nothing visible" step must produce NO screen change
- Every flash/squares/clear step must be visually distinct
- Display goes dark during low power and light sleep
- Device cold boots cleanly after power off → POWER press
