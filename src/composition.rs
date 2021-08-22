use image::imageops::FilterType;
use image::GenericImage;
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, Rgba};

use crate::random_comic_strips;

const COMPOSITION_WIDTH: u32 = 1200;
const COMPOSITION_HEIGHT: u32 = 825;
const COMPOSITION_MARGIN: u32 = 8;
const COMPOSITION_BACKGROUND: Rgba<u8> = Rgba([255, 0, 255, 255]);

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

    let comic_strips = random_comic_strips(3).await;
    assert!(comic_strips.len() > 0);

    let main_strip = &comic_strips[0];
    let main_image = main_strip.comics[0].image().await;
    let free_space = Rectangle {
        x: COMPOSITION_MARGIN,
        y: COMPOSITION_MARGIN,
        w: target.width() - 2 * COMPOSITION_MARGIN,
        h: target.height() - 2 * COMPOSITION_MARGIN,
    };
    let target_rectangle = best_fit_dimensions(free_space, &*main_image);
    resize_and_overlay(&mut target, &*main_image, target_rectangle);

    // if width == COMPOSITION_WIDTH {
    //     // Space down below
    // } else {
    //     // Space on the left
    // }

    DynamicImage::ImageRgba8(target)
}

fn best_fit_dimensions<J: GenericImageView>(rectangle: Rectangle, source: &J) -> Rectangle {
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

fn resize_and_overlay<I, J>(bottom: &mut I, top: &J, rect: Rectangle)
where
    I: GenericImage,
    J: GenericImageView<Pixel = I::Pixel>,
    I::Pixel: 'static,
{
    let resized_top = imageops::resize(top, rect.w, rect.h, FilterType::Lanczos3);

    imageops::overlay(bottom, &resized_top, rect.x, rect.y);
}
