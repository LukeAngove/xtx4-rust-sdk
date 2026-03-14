use embedded_graphics::{
    prelude::*,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    primitives::Rectangle,
    pixelcolor::BinaryColor,
    Pixel,
};

#[macro_export]
macro_rules! bit_buf {
    ($fill:expr; ($width:expr, $height:expr)) => {
        // Add '7' so we always add an extra byte, unless
        // it lines up exactly to a byte boundary.
        [$fill as u8; ($width * $height + 7) / 8]
    };
}

pub const STYLE_BLACK : BinaryColor = BinaryColor::Off;
pub const STYLE_WHITE : BinaryColor = BinaryColor::On;

pub struct Canvas<'a> {
    buf: &'a mut [u8],
    view: Rectangle,
    stride: u32,
}

impl<'a> Canvas<'a> {
    pub fn new(buf: &'a mut [u8], size: Size) -> Self {
        Self { buf, view: Rectangle::new(Point::new(0, 0), size), stride: size.width }
    }

    pub fn view<'b>(&'b mut self, view: Rectangle) -> Canvas<'b> where 'a: 'b {
        Canvas { buf: self.buf, view, stride: self.stride }
    }

    pub fn fill(&mut self, value: u8) {
        self.buf.fill(value);
    }

    pub fn buf(&self) -> &[u8] {
        self.buf
    }

    pub fn buf_mut(&mut self) -> &mut [u8] {
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
            let x = point.x as usize;
            let y = point.y as usize;
            if x < self.size().width as usize && y < self.size().height as usize {
                let px = (y + self.start().y as usize) * self.stride as usize + x + (self.start().x as usize);
                let byte = px / 8;
                let bit = px % 8;
                if color.is_on() {
                    self.buf[byte] |= 0x80 >> bit;
                } else {
                    self.buf[byte] &= !(0x80 >> bit);
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

