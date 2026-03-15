use embedded_graphics::{
    prelude::*,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    primitives::Rectangle,
    pixelcolor::BinaryColor,
    Pixel,
};

use xtx4_platform_interface::Buffer;

pub const STYLE_BLACK : BinaryColor = BinaryColor::Off;
pub const STYLE_WHITE : BinaryColor = BinaryColor::On;

pub struct Canvas<'a> {
    buf: &'a Buffer,
    view: Rectangle,
    stride: u32,
}

impl<'a> Canvas<'a> {
    pub fn new(buf: &'a Buffer, size: Size) -> Self {
        Self { buf, view: Rectangle::new(Point::new(0, 0), size), stride: size.width }
    }

    pub fn view<'b>(&'b self, view: Rectangle) -> Canvas<'b> where 'a: 'b {
        Canvas { buf: self.buf, view, stride: self.stride }
    }

    pub fn views<'b, const N: usize>(&'b self, views: &[Rectangle; N]) -> [Canvas<'b>; N] where 'a: 'b {
        core::array::from_fn(|i| {
            self.view(views[i])
        })
    }

    pub fn views_2d<'b, const ROWS: usize, const COLS: usize>(&'b self, views: &[[Rectangle; COLS]; ROWS]) -> [[Canvas<'b>; COLS]; ROWS] where 'a: 'b {
        core::array::from_fn(|i| {
            self.views(&views[i])
        })
    }

    pub fn fill(&self, value: u8) {
        let cells = self.buf.as_slice_of_cells();
        for c in cells {
            c.set(value);
        }
    }

    pub fn buf(&self) -> &Buffer {
        self.buf
    }

    pub fn view_port(&self) -> Rectangle {
        self.view
    }

    pub fn start(&self) -> Point {
        self.view.top_left
    }

    pub fn stride(&self) -> u32 {
        self.stride
    }
}

impl DrawTarget for Canvas<'_> {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where I: IntoIterator<Item = Pixel<BinaryColor>> {
        for Pixel(point, color) in pixels {
            let cells = self.buf.as_slice_of_cells();
            let x = point.x as usize;
            let y = point.y as usize;
            if x < self.size().width as usize && y < self.size().height as usize {
                let px = (y + self.start().y as usize) * self.stride as usize + x + (self.start().x as usize);
                let byte = px / 8;
                let bit = px % 8;
                if color.is_on() {
                    cells[byte].set(cells[byte].get() | (0x80 >> bit) as u8);
                } else {
                    cells[byte].set(cells[byte].get() & !(0x80 >> bit) as u8);
                }
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Canvas<'_> {
    fn size(&self) -> Size {
        self.view.size
    }
}


