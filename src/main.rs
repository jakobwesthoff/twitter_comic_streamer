mod composition;
mod twitter;

use composition::create_composition_image;
use egg_mode::user::UserID;
use egg_mode::Token;
use image::math::utils::clamp;
use image::{DynamicImage, ImageBuffer, Luma, Rgba};
use imageproc::definitions::Clamp;
use imageproc::filter::Kernel;
use rand::seq::SliceRandom;
use rocket::http::ContentType;
use rocket::response::content;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use twitter::ComicStrip;
use twitter::{access_token, twitter_refresh_task, UserComicCollection};

#[derive(Deserialize, Debug)]
struct Config {
    consumer_key: String,
    consumer_secret: String,
    access_token: String,
    access_token_secret: String,
    twitter_usernames: Vec<String>,
    twitter_refresh_interval: u64,
}

fn env_config() -> Config {
    match envy::from_env::<Config>() {
        Ok(c) => c,
        Err(error) => panic!("{:#?}", error),
    }
}

pub async fn random_comic_strips() -> Vec<Arc<ComicStrip>> {
    let collections = COLLECTION_ARC.get();
    let mut collection_refs: Vec<&Mutex<UserComicCollection>> = (*collections).iter().collect();
    collection_refs.shuffle(&mut rand::thread_rng());

    for shuffled_ref in collection_refs {
        let locked_collection = shuffled_ref.lock().await;
        if locked_collection.comic_strips.len() == 0 {
            continue;
        }

        let mut strips = locked_collection.comic_strips.clone();
        strips.shuffle(&mut rand::thread_rng());
        return strips;
    }

    return vec![];
}

fn png_image_data(image: &DynamicImage) -> Vec<u8> {
    let mut out_bytes: Vec<u8> = Vec::new();
    image
        .write_to(&mut out_bytes, image::ImageOutputFormat::Png)
        .unwrap();

    out_bytes
}

struct Dithering {
    normalization: u32,
    kernel: [[u32; 5]; 5],
}

const JARVIS_JUDICE_NINKE: Dithering = Dithering {
    normalization: 48,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 7, 5],
        [3, 5, 7, 5, 3],
        [1, 3, 5, 3, 1],
    ],
};

const FLOYD_STEINBERG: Dithering = Dithering {
    normalization: 16,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 7, 0],
        [0, 3, 5, 1, 0],
        [0, 0, 0, 0, 0],
    ],
};

const ATKINSON: Dithering = Dithering {
    normalization: 8,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 1, 1],
        [0, 1, 1, 1, 0],
        [0, 0, 1, 0, 0],
    ],
};

const JUST_MAP_NO_DITHER: Dithering = Dithering {
    normalization: 1,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
    ],
};

fn inkplate_image_data(image: &DynamicImage) -> Vec<u8> {
    let mut out_bytes: Vec<u8> = Vec::new();

    let grayscale = image.grayscale().to_luma8();
    let dithered = apply_error_diffusion(grayscale, FLOYD_STEINBERG);

    DynamicImage::ImageLuma8(dithered)
        .write_to(&mut out_bytes, image::ImageOutputFormat::Png)
        .unwrap();

    out_bytes
}

// const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
// const clampPixel = (value) => clamp(value, 0, 255);

fn is_inside_image(image: &ImageBuffer<Luma<u8>, Vec<u8>>, x: i64, y: i64) -> bool {
    !(x < 0 || y < 0 || x > image.width() as i64 - 1 || y > image.height() as i64 - 1)
}

fn kernel_by_delta(kernel: &[[u32; 5]; 5], dx: i64, dy: i64) -> u32 {
    let size = 5;
    let vx = (dx + (size - 1) / 2) as usize;
    let vy = (dy + (size - 1) / 2) as usize;

    // println!(
    //     "size: {}, dx: {}, dy: {}, vx: {}, vy: {}",
    //     size, dx, dy, vx, vy
    // );
    kernel[vy][vx]
}

fn apply_error_diffusion(
    mut image: ImageBuffer<Luma<u8>, Vec<u8>>,
    dither: Dithering,
) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let rows = 5;
    let cols = rows;
    for y in 0..image.height() {
        for x in 0..image.width() {
            let original = image.get_pixel(x, y).0[0];
            // Only look at the highest 3-bits
            let quantized_pixel = original & 0xe0;
            let quantization_error = original - quantized_pixel;

            let p = image.get_pixel_mut(x, y);
            p.0[0] = quantized_pixel;

            for dy in -2..=2 {
                for dx in -2..=2 {
                    // println!("dx: {}, dy: {}", dx, dy);
                    let kernel_value = kernel_by_delta(&dither.kernel, dx, dy);
                    // println!("{}", kernel_value);
                    let kx = i64::from(x) + dx;
                    let ky = i64::from(y) + dy;
                    if kernel_value != 0 && is_inside_image(&image, kx, ky) {
                        let p = image.get_pixel_mut(kx as u32, ky as u32);
                        let original = p.0[0];
                        let correction = ((quantization_error as f64 * kernel_value as f64)
                            / dither.normalization as f64)
                            .floor();

                        p.0[0] = clamp(original as f64 + correction, 0.0, 255.0) as u8;

                        // println!(
                        //     "before: {}, after: {}, correction: {}",
                        //     original, p.0[0], correction
                        // );
                    }
                }
            }
        }
    }

    image
}

#[rocket::get("/comic")]
async fn comic() -> Option<content::Custom<Vec<u8>>> {
    let comic_strips = random_comic_strips().await;
    if comic_strips.len() > 0 {
        return Some(content::Custom(
            ContentType::PNG,
            png_image_data(&*comic_strips[0].comics[0].image().await),
        ));
    }

    return None;
}

#[rocket::get("/composition")]
async fn comic_composition() -> Option<content::Custom<Vec<u8>>> {
    return Some(content::Custom(
        ContentType::PNG,
        png_image_data(&create_composition_image().await),
    ));
}

#[rocket::get("/inkplate")]
async fn comic_inkplate() -> Option<content::Custom<Vec<u8>>> {
    return Some(content::Custom(
        ContentType::PNG,
        inkplate_image_data(&create_composition_image().await),
    ));
}

static CONFIG: state::Storage<Config> = state::Storage::new();
static TOKEN: state::Storage<Token> = state::Storage::new();
static COLLECTION_ARC: state::Storage<Arc<Vec<Mutex<UserComicCollection>>>> = state::Storage::new();

#[tokio::main]
async fn main() {
    CONFIG.set(env_config());
    TOKEN.set(access_token());

    let mut user_collections = vec![];

    for twittername in CONFIG.get().twitter_usernames.iter() {
        user_collections.push(Mutex::new(UserComicCollection::new(UserID::ScreenName(
            twittername.into(),
        ))));
    }

    COLLECTION_ARC.set(Arc::new(user_collections));

    tokio::spawn(twitter_refresh_task(COLLECTION_ARC.get().clone()));

    rocket::build()
        .mount(
            "/",
            rocket::routes![comic, comic_composition, comic_inkplate],
        )
        .launch()
        .await
        .unwrap();
}
