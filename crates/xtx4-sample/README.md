# xtx4-sample

Sample application demonstrating SDK features. Used as the regression test
target (mock feature + golden PBM frames).

Maps device buttons to display operations:
- LEFT_OUTER: three black squares (fast refresh)
- LEFT_INNER: clears screen white
- RIGHT_INNER: SD card write/read + checkerboard display
- RIGHT_OUTER: white square
- SIDE_TOP: black/white column pairs
- SIDE_BOTTOM: progress bar
- POWER + face button combos: low power, sleep, power off
