use std::sync::Arc;

use image::imageops::FilterType;
use image::GenericImage;
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, Rgba};

use crate::{random_comic_strips, ComicStrip};

const COMPOSITION_WIDTH: u32 = 1200;
const COMPOSITION_HEIGHT: u32 = 825;
const COMPOSITION_MARGIN: u32 = 8;
const COMPOSITION_BACKGROUND: Rgba<u8> = Rgba([0, 0, 0, 255]);

#[derive(Debug, Copy, Clone)]
struct Rectangle {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

pub async fn create_composition_image() -> DynamicImage {
    let mut target = ImageBuffer::from_pixel(
        COMPOSITION_WIDTH,
        COMPOSITION_HEIGHT,
        COMPOSITION_BACKGROUND,
    );

    let comic_strips = random_comic_strips().await;
    assert!(comic_strips.len() > 0);

    let main_strip = &comic_strips[0];
    let main_image = main_strip.comics[0].image().await;
    let free_space = Rectangle {
        x: COMPOSITION_MARGIN,
        y: COMPOSITION_MARGIN,
        w: target.width() - 2 * COMPOSITION_MARGIN,
        h: target.height() - 2 * COMPOSITION_MARGIN,
    };
    let target_rectangle = best_fit_for_dimensions(free_space, &*main_image);
    resize_and_overlay(&mut target, &*main_image, target_rectangle);

    if target_rectangle.w == free_space.w {
        // Space down below
        let free_space = Rectangle {
            x: COMPOSITION_MARGIN,
            y: target_rectangle.y + target_rectangle.h + COMPOSITION_MARGIN,
            w: target_rectangle.w,
            h: target.height() - target_rectangle.y - target_rectangle.h - COMPOSITION_MARGIN * 2,
        };
        fill_row(&mut target, comic_strips, free_space).await
    } else {
        // Space to the right
        let free_space = Rectangle {
            x: target_rectangle.x + target_rectangle.w + COMPOSITION_MARGIN,
            y: COMPOSITION_MARGIN,
            w: target.width() - target_rectangle.x - target_rectangle.w - COMPOSITION_MARGIN * 2,
            h: target_rectangle.h 
        };
        fill_column(&mut target, comic_strips, free_space).await
    }

    DynamicImage::ImageRgba8(target)
}

async fn fill_row<I: GenericImage<Pixel = Rgba<u8>>>(
    target: &mut I,
    comic_strips: Vec<Arc<ComicStrip>>,
    row_space: Rectangle,
) {
    let mut draw_instructions: Vec<(Arc<DynamicImage>, Rectangle)> = vec![];
    let mut row_width = 0;
    let mut free_space = row_space;

    for comic_strip in comic_strips.iter() {
        let image = comic_strip.comics[0].image().await;
        let fit_rectangle = best_fit_for_height(free_space, &*image);

        if row_width + fit_rectangle.w > row_space.w {
            // This image does not fit anymore
            // stop here
            // break;
            // alternatively try other images of same artist
            continue;
        }

        let width_advance = fit_rectangle.w + COMPOSITION_MARGIN;
        row_width += width_advance;
        draw_instructions.push((image, fit_rectangle));
        free_space.w -= width_advance;
        free_space.x += width_advance;
    }

    // Adapt the margins to have a snap fit at the borders

    // The margin added during the calculation loop must be removed here again.
    let free_width = row_space.w - (row_width - COMPOSITION_MARGIN); 
    println!("free_width: {}, row_space.w: {}, row_width: {}", free_width, row_space.w, row_width);
    let margin_adjust = free_width as f64 / (draw_instructions.len() - 1) as f64;
    let mut index = 0;
    for (_, rect) in &mut draw_instructions {
        rect.x += (index as f64 * margin_adjust).round() as u32;
        index += 1;
    }

    for (top, rect) in draw_instructions {
        resize_and_overlay(target, &*top, rect);
    }
}

async fn fill_column<I: GenericImage<Pixel = Rgba<u8>>>(
    target: &mut I,
    comic_strips: Vec<Arc<ComicStrip>>,
    column_space: Rectangle,
) {
    let mut draw_instructions: Vec<(Arc<DynamicImage>, Rectangle)> = vec![];
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
        draw_instructions.push((image, fit_rectangle));
        free_space.h -= height_advance;
        free_space.y += height_advance;
    }

    // Adapt the margins to have a snap fit at the borders

    // The margin added during the calculation loop must be removed here again.
    let free_height = column_space.h - (column_height - COMPOSITION_MARGIN); 
    let margin_adjust = free_height as f64 / (draw_instructions.len() - 1) as f64;
    let mut index = 0;
    for (_, rect) in &mut draw_instructions {
        rect.y += (index as f64 * margin_adjust).round() as u32;
        index += 1;
    }

    for (top, rect) in draw_instructions {
        resize_and_overlay(target, &*top, rect);
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

fn resize_and_overlay<I, J>(bottom: &mut I, top: &J, rect: Rectangle)
where
    I: GenericImage,
    J: GenericImageView<Pixel = I::Pixel>,
    I::Pixel: 'static,
{
    let resized_top = imageops::resize(top, rect.w, rect.h, FilterType::Lanczos3);

    imageops::overlay(bottom, &resized_top, rect.x, rect.y);
}
