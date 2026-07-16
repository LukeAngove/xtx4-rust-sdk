# ssd1677-pbm

In-memory mock transport for the SSD1677. Captures every display refresh as a
PBM file in `/tmp/xtx4_frames/`. Used for regression testing.

Implements `DisplayInterface` with full RAM bank emulation, LUT processing,
and frame rotation (landscape → portrait).
