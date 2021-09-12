use image::DynamicImage;

use crate::dithering;

pub fn png(image: &DynamicImage) -> Vec<u8> {
  let mut out_bytes: Vec<u8> = Vec::new();
  image
    .write_to(&mut out_bytes, image::ImageOutputFormat::Png)
    .unwrap();

  out_bytes
}

pub fn inkplate_png(image: &DynamicImage) -> Vec<u8> {
  let mut out_bytes: Vec<u8> = Vec::new();

  let dithered = dithering::quantize_to_3bit(image, dithering::floyd_steinberg());

  DynamicImage::ImageLuma8(dithered)
    .write_to(&mut out_bytes, image::ImageOutputFormat::Png)
    .unwrap();

  out_bytes
}

#[inline(always)]
fn is_odd(value: u32) -> bool {
  value & 0x1 == 0x1
}

pub fn inkplate_raw(image: &DynamicImage) -> Vec<u8> {
  let dithered = dithering::quantize_to_3bit(image, dithering::jarvis_judice_ninke());
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
