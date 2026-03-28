use embedded_graphics::{geometry::Size, prelude::*, primitives::Rectangle};

pub trait Split: Sized {
    fn split_vert<const N: usize>(&self, splits: &[u32; N]) -> [Self; N];
    fn split_horz<const N: usize>(&self, splits: &[u32; N]) -> [Self; N];
    fn inset(&self, margin: u32) -> Self;
    fn split_grid_custom<const ROWS: usize, const COLS: usize>(
        &self,
        row_splits: &[u32; ROWS],
        col_splits: &[u32; COLS],
    ) -> [[Self; COLS]; ROWS];
    fn split_grid_even<'b, const ROWS: usize, const COLS: usize>(&'b self) -> [[Self; COLS]; ROWS];
}

impl Split for Rectangle {
    fn split_vert<'b, const N: usize>(&'b self, splits: &[u32; N]) -> [Rectangle; N] {
        split_vert(&self, splits)
    }

    fn split_horz<'b, const N: usize>(&'b self, splits: &[u32; N]) -> [Rectangle; N] {
        split_horz(&self, splits)
    }

    fn inset<'b>(&'b self, margin: u32) -> Rectangle {
        let top_left = self.top_left + Point::new(margin as i32, margin as i32);
        let size = self.size - Size::new(margin, margin);

        Rectangle::new(top_left, size)
    }

    fn split_grid_custom<'b, const ROWS: usize, const COLS: usize>(
        &'b self,
        row_splits: &[u32; ROWS],
        col_splits: &[u32; COLS],
    ) -> [[Rectangle; COLS]; ROWS] {
        let row_rects = split_vert_rects(self, row_splits);

        core::array::from_fn(|row| split_horz_rects(&row_rects[row], col_splits))
    }

    fn split_grid_even<'b, const ROWS: usize, const COLS: usize>(
        &'b self,
    ) -> [[Rectangle; COLS]; ROWS] {
        self.split_grid_custom(&[1; ROWS], &[1; COLS])
    }
}

fn calc_segments(size: u32, ratios: &[u32]) -> (u32, u32) {
    let total: u32 = ratios.iter().sum();
    let excess = size % total;
    let pixel_segments = size / total;

    (pixel_segments, excess)
}

fn add_to_top_n<const LEN: usize>(arr: &mut [u32; LEN], n: u32) {
    // Build an array of indices sorted by value descending
    let mut indices = core::array::from_fn::<usize, LEN, _>(|i| i);
    indices.sort_unstable_by(|&a, &b| arr[b].cmp(&arr[a]));

    // Increment the top n
    for &i in indices.iter().take(n as usize) {
        arr[i] += 1;
    }
    // TODO assert sum(heights) == full_height
}

fn make_even_split<const N: usize>(size: u32, ratios: &[u32; N]) -> [u32; N] {
    let (pixel_segments, excess) = calc_segments(size, ratios);

    let mut splits = core::array::from_fn(|i| ratios[i] * pixel_segments);

    add_to_top_n(&mut splits, excess);

    splits
}

fn split_vert<const N: usize>(source: &Rectangle, ratios: &[u32; N]) -> [Rectangle; N] {
    let full_height = source.size.height;

    let heights = make_even_split(full_height, ratios);

    split_vert_rects(source, &heights)
}

fn split_vert_rects<const N: usize>(full_rect: &Rectangle, heights: &[u32; N]) -> [Rectangle; N] {
    let mut y = full_rect.top_left.y;

    core::array::from_fn(|i| {
        let rect = Rectangle::new(
            Point::new(full_rect.top_left.x, y),
            Size::new(full_rect.size.width, heights[i]),
        );
        y += heights[i] as i32;
        rect
    })
}

fn split_horz<const N: usize>(source: &Rectangle, ratios: &[u32; N]) -> [Rectangle; N] {
    let full_width = source.size.width;

    let widths = make_even_split(full_width, ratios);

    split_horz_rects(source, &widths)
}

fn split_horz_rects<const N: usize>(full_rect: &Rectangle, widths: &[u32; N]) -> [Rectangle; N] {
    let mut x = full_rect.top_left.x;

    core::array::from_fn(|i| {
        let rect = Rectangle::new(
            Point::new(x, full_rect.top_left.y),
            Size::new(widths[i], full_rect.size.height),
        );
        x += widths[i] as i32;
        rect
    })
}
