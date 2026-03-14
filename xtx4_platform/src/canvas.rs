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

    pub fn split_vert<'b>(&'b self, top_ratio: u32, bottom_ratio: u32) -> (Canvas<'b>, Canvas<'b>) where 'a: 'b {
        let full_rect = self.view;
        let full_height = full_rect.size.height;
        let total = top_ratio + bottom_ratio;
        let excess = full_height % total;
        let pixel_segments = full_height / total;

        let mut top_height = pixel_segments * top_ratio;
        let mut bottom_height = pixel_segments * bottom_ratio;

        if top_ratio > bottom_ratio {
            top_height += excess;
        } else {
            bottom_height += excess;
        }

        // TODO assert top_height + bottom_height == full_height

        let top_rect = Rectangle::new(full_rect.top_left, Size::new(full_rect.size.width, top_height));
        let bottom_rect = Rectangle::new(Point::new(full_rect.top_left.x, full_rect.top_left.y + top_height as i32), Size::new(full_rect.size.width, bottom_height));

        let top = Canvas { buf: &self.buf, view: top_rect, stride: self.stride };
        let bottom =  Canvas { buf: &self.buf, view: bottom_rect, stride: self.stride };

        (top, bottom)
    }

    pub fn split_horz<'b>(&'b self, left_ratio: u32, right_ratio: u32) -> (Canvas<'b>, Canvas<'b>) where 'a: 'b {
        let full_rect = self.view;
        let full_width = full_rect.size.width;
        let total = left_ratio + right_ratio;
        let excess = full_width % total;
        let pixel_segments = full_width / total;

        let mut left_width = pixel_segments * left_ratio;
        let mut right_width = pixel_segments * right_ratio;

        if left_ratio > right_ratio {
            left_width += excess;
        } else {
            right_width += excess;
        }

        // TODO assert left_width + right_width == full_width

        let left_rect = Rectangle::new(full_rect.top_left, Size::new(left_width, full_rect.size.height));
        let right_rect = Rectangle::new(Point::new(full_rect.top_left.x + left_width as i32, full_rect.top_left.y), Size::new(right_width, full_rect.size.height));

        let left = Canvas { buf: &self.buf, view: left_rect, stride: self.stride };
        let right =  Canvas { buf: &self.buf, view: right_rect, stride: self.stride };

        (left, right)
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

