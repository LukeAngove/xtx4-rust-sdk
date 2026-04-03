use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    Pixel,
};

use xtx4_platform_interface::{Buffer, DrawTransform};
use core::marker::PhantomData;

pub const STYLE_BLACK: BinaryColor = BinaryColor::Off;
pub const STYLE_WHITE: BinaryColor = BinaryColor::On;

pub struct Canvas<'a, Transform: DrawTransform> {
    buf: &'a Buffer,
    view: Rectangle,
    stride: Size,
    _token: PhantomData<Transform>,
}

impl<'a, Transform: DrawTransform> Canvas<'a, Transform> {
    pub fn new(buf: &'a Buffer, size: Size) -> Self {
        // round stride up to multiples of 8 so we can be byte aligned.
        //let stride = Size::new((size.width+7)/8, (size.height+7)/8);
        let stride = size;
        if buf.as_slice_of_cells().len() < ((stride.width as usize * stride.height as usize)/8) {
            panic!("Buffer too small for stride! Ensure you accounted for byte alignment for each row!");
        }
        Self {
            buf,
            view: Rectangle::new(Point::new(0, 0), size),
            stride: stride,
            _token: PhantomData,
        }
    }

    pub fn view<'b>(&'b self, view: Rectangle) -> Canvas<'b, Transform>
    where
        'a: 'b,
    {
        Canvas::<Transform> {
            buf: self.buf,
            view,
            stride: self.stride,
            _token: PhantomData,
        }
    }

    pub fn views<'b, const N: usize>(&'b self, views: &[Rectangle; N]) -> [Canvas<'b, Transform>; N]
    where
        'a: 'b,
    {
        core::array::from_fn(|i| self.view(views[i]))
    }

    pub fn views_2d<'b, const ROWS: usize, const COLS: usize>(
        &'b self,
        views: &[[Rectangle; COLS]; ROWS],
    ) -> [[Canvas<'b, Transform>; COLS]; ROWS]
    where
        'a: 'b,
    {
        core::array::from_fn(|i| self.views(&views[i]))
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
}

impl <Transform: DrawTransform> DrawTarget for Canvas<'_, Transform> {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<BinaryColor>>,
    {
        // All values in the 'physical' space are marked with 'p_'
        // This includes any rotation or scaling for the target hardware.

        let cells = self.buf.as_slice_of_cells();
        let max_pix = cells.len()*8;

        let p_stride = Transform::stride(self.stride.width.try_into().unwrap(), self.stride.height.try_into().unwrap()) as usize;

        let start = self.start();
        let swidth: u16 = self.stride.width.try_into().unwrap();
        let sheight: u16 = self.stride.height.try_into().unwrap();

        for Pixel(point, color) in pixels {
            // Put in top level space.
            let point = point + start;

            if let Some((p_x,p_y)) = Transform::apply(point.x as u16, point.y as u16, swidth, sheight) {
                // Use 'usize' so we don't overflow; this can be big.
                let p_idx = (p_y as usize * p_stride) + p_x as usize;

                if p_idx < max_pix {
                    let byte = p_idx / 8;
                    let bit = p_idx % 8;
                    if color.is_on() {
                        cells[byte].set(cells[byte].get() | (0x80 >> bit) as u8);
                    } else {
                        cells[byte].set(cells[byte].get() & !(0x80 >> bit) as u8);
                    }
                } else {
                    panic!("Tried to write to image out of bounds: {}, max: {}, P:({}, {}), ({}, {}), stride: {}, start: ({}, {})", p_idx, max_pix, p_x, p_y, point.x, point.y, p_stride, start.x, start.y);
                }
            }
        }
        Ok(())
    }
}

impl <Transform: DrawTransform> OriginDimensions for Canvas<'_, Transform> {
    fn size(&self) -> Size {
        self.view.size
    }
}
