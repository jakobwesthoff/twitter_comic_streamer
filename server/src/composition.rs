use std::sync::Arc;

use image::imageops::FilterType;
use image::GenericImage;
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, Rgba};

use crate::comic_image::ComicImage;
use crate::layout::{CalculateLayout, ColumnLayout, Layout, RowLayout, SingleLayout};
use crate::random_comic_strips;

const COMPOSITION_WIDTH: f64 = 1200.0;
const COMPOSITION_HEIGHT: f64 = 825.0;
const COMPOSITION_MARGIN: f64 = 8.0;
const COMPOSITION_SPLIT_MIN: f64 = 0.30;
const COMPOSITION_BACKGROUND: Rgba<u8> = Rgba([255, 255, 255, 255]);

#[derive(Debug, Copy, Clone)]
struct Rectangle {
  x: u32,
  y: u32,
  w: u32,
  h: u32,
}

#[derive(Debug)]
struct Size {
  w: f64,
  h: f64,
}

impl Size {
  pub fn new(w: f64, h: f64) -> Self {
    Self { w, h }
  }
}

struct DrawingInstruction {
  image: Arc<ComicImage>,
  area: Rectangle,
}

pub async fn create_composition_image() -> DynamicImage {
  let comic_strips = random_comic_strips().await;
  assert!(comic_strips.len() > 0);

  let primary_strip = &comic_strips[0];
  let primary_image = primary_strip.comics[0].image();
  let primary_size = size_to_fit(
    &*primary_image,
    Size::new(COMPOSITION_WIDTH, COMPOSITION_HEIGHT),
  );

  // Decide between RowLayout, ColumnLayout or SingleLayout
  let layout;
  if primary_size.h < COMPOSITION_HEIGHT
    && COMPOSITION_HEIGHT - primary_size.h > COMPOSITION_HEIGHT * COMPOSITION_SPLIT_MIN
    && comic_strips.len() > 1
  {
    let mut filled_width = 0.0;
    let mut secondary_images: Vec<Arc<ComicImage>> = vec![];
    for secondary_strip in &comic_strips[1..] {
      let secondary_image = secondary_strip.comics[0].image();
      let secondary_size = size_to_fit(
        &*secondary_image,
        Size::new(COMPOSITION_WIDTH, COMPOSITION_HEIGHT - primary_size.h),
      );
      if filled_width + secondary_size.w <= COMPOSITION_WIDTH {
        secondary_images.push(secondary_image.clone());
        filled_width += secondary_size.w;
      }
    }
    layout = Layout::from(RowLayout::new_with_margin(
      primary_image,
      secondary_images,
      COMPOSITION_MARGIN,
    ));
  } else if primary_size.w < COMPOSITION_WIDTH
    && COMPOSITION_WIDTH - primary_size.w > COMPOSITION_WIDTH * COMPOSITION_SPLIT_MIN
    && comic_strips.len() > 1
  {
    let mut filled_height = 0.0;
    let mut secondary_images: Vec<Arc<ComicImage>> = vec![];
    for secondary_strip in &comic_strips[1..] {
      let secondary_image = secondary_strip.comics[0].image();
      let secondary_size = size_to_fit(
        &*secondary_image,
        Size::new(COMPOSITION_WIDTH - primary_size.w, COMPOSITION_HEIGHT),
      );
      if filled_height + secondary_size.h <= COMPOSITION_HEIGHT {
        secondary_images.push(secondary_image.clone());
        filled_height += secondary_size.h;
      }
    }
    layout = Layout::from(ColumnLayout::new_with_margin(
      primary_image,
      secondary_images,
      COMPOSITION_MARGIN,
    ));
  } else {
    layout = Layout::from(SingleLayout::new_with_margin(primary_image.clone(), COMPOSITION_MARGIN));
  }

  let mut target = ImageBuffer::from_pixel(
    COMPOSITION_WIDTH as u32,
    COMPOSITION_HEIGHT as u32,
    COMPOSITION_BACKGROUND,
  );

  let layout_instructions = layout.calculate();
  let instructions = layout_instructions.iter().map(|instr| DrawingInstruction {
    image: instr.image.clone(),
    area: Rectangle {
      x: instr.x,
      y: instr.y,
      w: instr.w,
      h: instr.h,
    },
  });

  for instr in instructions {
    resize_and_overlay(&mut target, &instr.image.dynamic_image(), instr.area);
  }

  DynamicImage::ImageRgba8(target)
 }

fn size_to_fit(image: &ComicImage, max_size: Size) -> Size {
  let width = image.width();
  let height = image.height();
  let aspect_ratio: f64 = width as f64 / height as f64;

  let max_width = max_size.w;
  let max_height = max_size.h;

  if max_width / aspect_ratio > max_height {
    // Height is maximized
    Size {
      w: (max_height * aspect_ratio).floor(),
      h: max_height,
    }
  } else {
    // Width is maximized
    Size {
      w: max_width,
      h: (max_width / aspect_ratio).floor(),
    }
  }
}

#[inline(always)]
fn resize_and_overlay<I, J>(bottom: &mut I, top: &J, area: Rectangle)
where
  I: GenericImage,
  J: GenericImageView<Pixel = I::Pixel>,
  I::Pixel: 'static,
{
  let resized_top = imageops::resize(top, area.w, area.h, FilterType::Lanczos3);

  imageops::overlay(bottom, &resized_top, area.x, area.y);
}
