use std::io::Cursor;

use image::png::PngEncoder;
use image::{ColorType, DynamicImage, GenericImageView, ImageFormat};

use crate::dithering;

#[inline(always)]
fn is_odd(value: u32) -> bool {
  value & 0x1 == 0x1
}

#[derive(Debug)]
pub struct ComicImage {
  data: Vec<u8>,
  width: u32,
  height: u32,
  color: ColorType,
}

impl From<DynamicImage> for ComicImage {
  fn from(image: DynamicImage) -> Self {
    let width = image.width();
    let height = image.height();
    let color = image.color();

    // Create new compressed PNG for storage
    let mut data: Vec<u8> = Vec::new();
    let encoder = PngEncoder::new_with_quality(
      &mut data,
      image::png::CompressionType::Fast,
      image::png::FilterType::Sub,
    );

    encoder
      .encode(image.as_bytes(), width, height, color)
      .unwrap();

    Self {
      data,
      width,
      height,
      color,
    }
  }
}

impl ComicImage {
  pub fn dynamic_image(&self) -> DynamicImage {
    let mut img = image::io::Reader::new(Cursor::new(&self.data));
    img.set_format(ImageFormat::Png);
    img.decode().unwrap()
  }

  pub fn png_image(&self) -> Vec<u8> {
    self.data.clone()
  }

  pub fn dithered_png_image(&self) -> Vec<u8> {
    let mut out_bytes: Vec<u8> = Vec::new();

    let dithered = dithering::quantize_to_3bit(&self.dynamic_image(), dithering::floyd_steinberg());

    DynamicImage::ImageLuma8(dithered)
      .write_to(&mut out_bytes, image::ImageOutputFormat::Png)
      .unwrap();

    out_bytes
  }

  pub fn inkplate_image(&self) -> Vec<u8> {
    let dithered =
      dithering::quantize_to_3bit(&self.dynamic_image(), dithering::jarvis_judice_ninke());
    let (width, height) = dithered.dimensions();

    // Minimize possible reallocations
    let mut out_bytes: Vec<u8> = Vec::with_capacity((width * height / 2) as usize);

    // We are encoding 2 3bit pixels per bit one in the upper byte, one in the
    // lower. Furthermore we are always starting a new byte at the beginning of
    // each row. Therefore the last one might need padding if the width is odd
    let odd_width = is_odd(width);

    for y in 0..height {
      let mut current_byte: u8 = 0x0;
      for x in 0..width {
        let p = dithering::get_pixel(&dithered, x, y);

        if is_odd(x) {
          // First of two pixels (high nible)
          current_byte = p & 0xf0
        } else {
          // Second of two pixels (low nible)
          current_byte = current_byte | (p >> 4);

          // Write finished byte
          out_bytes.push(current_byte);
        }

        if odd_width && x == width - 1 {
          // Write out last byte with padding before switching lines.
          out_bytes.push(current_byte);
        }
      }
    }

    out_bytes
  }

  pub fn width(&self) -> u32 {
    self.width
  }

  pub fn height(&self) -> u32 {
    self.height
  }

  pub fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  pub fn color(&self) -> ColorType {
    self.color
  }
}
