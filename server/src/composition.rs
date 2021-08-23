use std::sync::Arc;

use image::imageops::FilterType;
use image::GenericImage;
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, Rgba};

use crate::{random_comic_strips, ComicStrip};

const COMPOSITION_WIDTH: u32 = 1200;
const COMPOSITION_HEIGHT: u32 = 825;
const COMPOSITION_MARGIN: u32 = 8;
const COMPOSITION_SPLIT_MIN: f64 = 0.25;
const MINIMUM_IMAGES_IN_SPLIT: usize = 2;
const COMPOSITION_BACKGROUND: Rgba<u8> = Rgba([0, 0, 0, 255]);
const COMPOSITION_SINGLE_BACKGROUND: Rgba<u8> = Rgba([255, 255, 255, 255]);

#[derive(Debug, Copy, Clone)]
struct Rectangle {
  x: u32,
  y: u32,
  w: u32,
  h: u32,
}

struct DrawingInstruction {
  image: Arc<DynamicImage>,
  area: Rectangle,
}

pub async fn create_composition_image() -> DynamicImage {
  let comic_strips = random_comic_strips().await;
  assert!(comic_strips.len() > 0);

  let main_strip = &comic_strips[0];
  let main_image = main_strip.comics[0].image().await;
  let free_space = Rectangle {
    x: COMPOSITION_MARGIN,
    y: COMPOSITION_MARGIN,
    w: COMPOSITION_WIDTH - 2 * COMPOSITION_MARGIN,
    h: COMPOSITION_HEIGHT - 2 * COMPOSITION_MARGIN,
  };
  let target_rectangle = best_fit_for_dimensions(free_space, &*main_image);
  let main_instruction = DrawingInstruction {
    image: main_image,
    area: target_rectangle,
  };

  let mut drawing_instructions = vec![main_instruction];

  if target_rectangle.w == free_space.w {
    // Space down below
    let free_space = Rectangle {
      x: COMPOSITION_MARGIN,
      y: target_rectangle.y + target_rectangle.h + COMPOSITION_MARGIN,
      w: target_rectangle.w,
      h: COMPOSITION_HEIGHT - target_rectangle.y - target_rectangle.h - COMPOSITION_MARGIN * 2,
    };

    if free_space.h > (COMPOSITION_SPLIT_MIN * COMPOSITION_HEIGHT as f64).floor() as u32 {
      drawing_instructions.extend(fill_row(&comic_strips[1..], free_space).await);
    }
  } else {
    // Space to the right
    let free_space = Rectangle {
      x: target_rectangle.x + target_rectangle.w + COMPOSITION_MARGIN,
      y: COMPOSITION_MARGIN,
      w: COMPOSITION_WIDTH - target_rectangle.x - target_rectangle.w - COMPOSITION_MARGIN * 2,
      h: target_rectangle.h,
    };

    if free_space.w > (COMPOSITION_SPLIT_MIN * COMPOSITION_WIDTH as f64).floor() as u32 {
      drawing_instructions.extend(fill_column(&comic_strips[1..], free_space).await);
    }
  }

  let mut background_color = COMPOSITION_BACKGROUND;

  if drawing_instructions.len() == 1 {
    // No filler was generated, either due to lack of images, or due to
    // SPLIT_MIN not reached
    // Center the primary instead.
    let dx = ((COMPOSITION_WIDTH - drawing_instructions[0].area.w) as f64 / 2.0).round() as u32;
    let dy = ((COMPOSITION_HEIGHT - drawing_instructions[0].area.h) as f64 / 2.0).round() as u32;

    drawing_instructions[0].area.x = dx;
    drawing_instructions[0].area.y = dy;

    // and switch background color
    background_color = COMPOSITION_SINGLE_BACKGROUND;
  }

  let mut target = ImageBuffer::from_pixel(COMPOSITION_WIDTH, COMPOSITION_HEIGHT, background_color);

  for instr in drawing_instructions {
    resize_and_overlay(&mut target, &*instr.image, instr.area);
  }

  DynamicImage::ImageRgba8(target)
}

async fn fill_row(
  comic_strips: &[Arc<ComicStrip>],
  row_space: Rectangle,
) -> Vec<DrawingInstruction> {
  let mut draw_instructions = vec![];
  let mut row_width = 0;
  let mut free_space = row_space;

  for comic_strip in comic_strips.iter() {
    let image = comic_strip.comics[0].image().await;
    let fit_rectangle = best_fit_for_height(free_space, &*image);

    if row_width + fit_rectangle.w > row_space.w {
      // This image does not fit anymore
      // stop here
      break;
      // alternatively try other images of same artist
      // continue;
    }

    let width_advance = fit_rectangle.w + COMPOSITION_MARGIN;
    row_width += width_advance;
    draw_instructions.push(DrawingInstruction {
      image,
      area: fit_rectangle,
    });
    free_space.w -= width_advance;
    free_space.x += width_advance;
  }

  // Center the row
  // The margin added during the calculation loop must be removed here again.
  let free_width = row_space.w - (row_width - COMPOSITION_MARGIN);
  let margin_adjust = free_width as f64 / 2.0;
  for instr in &mut draw_instructions {
    instr.area.x = (instr.area.x as f64 + margin_adjust).round() as u32;
  }

  if draw_instructions.len() < MINIMUM_IMAGES_IN_SPLIT {
    vec![]
  } else {
    draw_instructions
  }
}

async fn fill_column(
  comic_strips: &[Arc<ComicStrip>],
  column_space: Rectangle,
) -> Vec<DrawingInstruction> {
  let mut draw_instructions = vec![];
  let mut column_height = 0;
  let mut free_space = column_space;

  for comic_strip in comic_strips.iter() {
    let image = comic_strip.comics[0].image().await;
    let fit_rectangle = best_fit_for_width(free_space, &*image);

    if column_height + fit_rectangle.h > column_space.h {
      // This image does not fit anymore
      // stop here
      break;
      // alternatively try other images of same artist
      // continue;
    }

    let height_advance = fit_rectangle.h + COMPOSITION_MARGIN;
    column_height += height_advance;
    draw_instructions.push(DrawingInstruction {
      image,
      area: fit_rectangle,
    });
    free_space.h -= height_advance;
    free_space.y += height_advance;
  }

  if draw_instructions.len() == 0 {
    // No match at all. Calculations would cause overflow. Just return early
    return vec![];
  }

  // Center the column
  // The margin added during the calculation loop must be removed here again.
  let free_height = column_space.h - (column_height - COMPOSITION_MARGIN);
  let margin_adjust = free_height as f64 / 2.0;
  for instr in &mut draw_instructions {
    instr.area.y = (instr.area.y as f64 + margin_adjust).round() as u32;
  }

  if draw_instructions.len() < MINIMUM_IMAGES_IN_SPLIT {
    vec![]
  } else {
    draw_instructions
  }
}

fn best_fit_for_dimensions<J: GenericImageView>(rectangle: Rectangle, source: &J) -> Rectangle {
  let (width, height) = source.dimensions();
  let aspect_ratio: f64 = width as f64 / height as f64;

  let max_width = rectangle.w;
  let max_height = rectangle.h;

  if max_width as f64 / aspect_ratio > max_height as f64 {
    Rectangle {
      w: (max_height as f64 * aspect_ratio).floor() as u32,
      h: max_height,
      x: rectangle.x,
      y: rectangle.y,
    }
  } else {
    Rectangle {
      w: max_width,
      h: (max_width as f64 / aspect_ratio).floor() as u32,
      x: rectangle.x,
      y: rectangle.y,
    }
  }
}

fn best_fit_for_height<J: GenericImageView>(rectangle: Rectangle, source: &J) -> Rectangle {
  let (width, height) = source.dimensions();
  let aspect_ratio: f64 = width as f64 / height as f64;

  let max_height = rectangle.h;

  Rectangle {
    w: (max_height as f64 * aspect_ratio).floor() as u32,
    h: max_height,
    x: rectangle.x,
    y: rectangle.y,
  }
}

fn best_fit_for_width<J: GenericImageView>(rectangle: Rectangle, source: &J) -> Rectangle {
  let (width, height) = source.dimensions();
  let aspect_ratio: f64 = width as f64 / height as f64;

  let max_width = rectangle.w;

  Rectangle {
    w: max_width,
    h: (max_width as f64 / aspect_ratio).floor() as u32,
    x: rectangle.x,
    y: rectangle.y,
  }
}

fn resize_and_overlay<I, J>(bottom: &mut I, top: &J, area: Rectangle)
where
  I: GenericImage,
  J: GenericImageView<Pixel = I::Pixel>,
  I::Pixel: 'static,
{
  let resized_top = imageops::resize(top, area.w, area.h, FilterType::Lanczos3);

  imageops::overlay(bottom, &resized_top, area.x, area.y);
}
