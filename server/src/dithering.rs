use image::{DynamicImage, ImageBuffer, Luma};

type Kernel5x5 = [[u32; 5]; 5];
#[derive(Debug, Copy, Clone)]
pub struct Dithering {
  normalization: u32,
  kernel: Kernel5x5,
}

impl Dithering {
  #[inline(always)]
  fn new(kernel: Kernel5x5) -> Self {
    let mut normalization: u32 = 0;
    for row in 0..5 {
      for col in 0..5 {
        normalization += kernel[row][col];
      }
    }

    Dithering {
      kernel,
      normalization,
    }
  }
}

#[allow(dead_code)]
#[inline(always)]
pub fn jarvis_judice_ninke() -> Dithering {
  Dithering::new([
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 7, 5],
    [3, 5, 7, 5, 3],
    [1, 3, 5, 3, 1],
  ])
}

#[allow(dead_code)]
#[inline(always)]
pub fn floyd_steinberg() -> Dithering {
  Dithering::new([
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 7, 0],
    [0, 3, 5, 1, 0],
    [0, 0, 0, 0, 0],
  ])
}

#[allow(dead_code)]
#[inline(always)]
pub fn atkinson() -> Dithering {
  Dithering::new([
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 1, 1],
    [0, 1, 1, 1, 0],
    [0, 0, 1, 0, 0],
  ])
}

#[allow(dead_code)]
#[inline(always)]
pub fn none() -> Dithering {
  Dithering::new([
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
  ])
}

#[inline(always)]
fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
  if value < min {
    min
  } else if value > max {
    max
  } else {
    value
  }
}

#[inline(always)]
fn assert_within_range<T>(value: T, min: T, max: T)
where
  T: PartialOrd,
  T: std::fmt::Debug,
{
  if value < min || value > max {
    panic!(
      "Given value is not within range: {:?} < {:?} < {:?}",
      min, value, max
    );
  }
}

#[inline(always)]
fn is_inside_image(image: &ImageBuffer<Luma<u8>, Vec<u8>>, x: i64, y: i64) -> bool {
  !(x < 0 || y < 0 || x > image.width() as i64 - 1 || y > image.height() as i64 - 1)
}

#[inline(always)]
fn kernel_by_delta(kernel: &Kernel5x5, dx: i64, dy: i64) -> u32 {
  assert_within_range(dx, -2, 2);
  assert_within_range(dy, -2, 2);

  let vx = (dx + 2) as usize;
  let vy = (dy + 2) as usize;

  kernel[vy][vx]
}

#[inline(always)]
fn quantize_pixel_3bit(pixel: u8) -> (u8, u8) {
  // Only look at the highest 3-bits
  let quantized_pixel = pixel & 0xe0;
  let quantization_error = pixel - quantized_pixel;

  (quantized_pixel, quantization_error)
}

#[inline(always)]
pub fn get_pixel(image: &ImageBuffer<Luma<u8>, Vec<u8>>, x: u32, y: u32) -> u8 {
  image.get_pixel(x, y).0[0]
}

#[inline(always)]
fn set_pixel(image: &mut ImageBuffer<Luma<u8>, Vec<u8>>, x: u32, y: u32, new_pixel: u8) {
  image.get_pixel_mut(x, y).0[0] = new_pixel;
}

fn apply_error_diffusion(
  mut image: ImageBuffer<Luma<u8>, Vec<u8>>,
  dither: Dithering,
) -> ImageBuffer<Luma<u8>, Vec<u8>> {
  for y in 0..image.height() {
    for x in 0..image.width() {
      // Quantize and set pixel value
      let (quantized_pixel, quantization_error) = quantize_pixel_3bit(get_pixel(&image, x, y));
      set_pixel(&mut image, x, y, quantized_pixel);

      // Apply quantization error to surrounding pixels according to diffusion kernel
      for dy in -2..=2 {
        for dx in -2..=2 {
          let kernel_value = kernel_by_delta(&dither.kernel, dx, dy);

          let kx = i64::from(x) + dx;
          let ky = i64::from(y) + dy;

          if kernel_value != 0 && is_inside_image(&image, kx, ky) {
            let original = get_pixel(&image, kx as u32, ky as u32);
            let correction = ((quantization_error as f64 * kernel_value as f64)
              / dither.normalization as f64)
              .floor();

            set_pixel(
              &mut image,
              kx as u32,
              ky as u32,
              clamp(original as f64 + correction, 0.0, 255.0) as u8,
            );
          }
        }
      }
    }
  }

  image
}

pub fn quantize_to_3bit(
  image: &DynamicImage,
  dithering: Dithering,
) -> ImageBuffer<Luma<u8>, Vec<u8>> {
  let grayscale = image.grayscale().to_luma8();
  apply_error_diffusion(grayscale, dithering)
}
