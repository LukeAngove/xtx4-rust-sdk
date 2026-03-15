use crate::canvas::Canvas;
use crate::rect_split::Split;

impl<'a> Canvas<'a> {
    pub fn split_vert<'b, const N: usize>(&'b self, splits: &[u32; N]) -> [Canvas<'b>; N] where 'a: 'b {
        self.views(&self.view_port().split_vert(splits))
    }

    pub fn split_horz<'b, const N: usize>(&'b self, splits: &[u32; N]) -> [Canvas<'b>; N] where 'a: 'b {
        self.views(&self.view_port().split_horz(splits))
    }

    pub fn inset<'b>(&'b self, margin: u32) -> Canvas<'b> where 'a: 'b {
        self.view(self.view_port().inset(margin))
    }

    pub fn split_grid_custom<'b, const ROWS: usize, const COLS: usize>(&'b self, row_splits: &[u32; ROWS], col_splits: &[u32; COLS]) -> [[Canvas<'b>; COLS]; ROWS] where 'a: 'b {
        self.views_2d(&self.view_port().split_grid_custom(row_splits, col_splits))
    }

    pub fn split_grid_even<'b, const ROWS: usize, const COLS: usize>(&'b self) -> [[Canvas<'b>; COLS]; ROWS] where 'a: 'b {
        self.views_2d(&self.view_port().split_grid_even())
    }
}
