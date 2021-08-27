mod composition;
mod dithering;
mod layout;
mod twitter;

use composition::create_composition_image;
use egg_mode::user::UserID;
use egg_mode::Token;
use image::DynamicImage;
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

fn inkplate_png_image_data(image: &DynamicImage) -> Vec<u8> {
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

fn inkplate_raw_image_data(image: &DynamicImage) -> Vec<u8> {
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

#[rocket::get("/comic/color")]
async fn comic_color() -> Option<content::Custom<Vec<u8>>> {
  return Some(content::Custom(
    ContentType::PNG,
    png_image_data(&create_composition_image().await),
  ));
}

#[rocket::get("/comic/grayscale")]
async fn comic_grayscale() -> Option<content::Custom<Vec<u8>>> {
  return Some(content::Custom(
    ContentType::PNG,
    inkplate_png_image_data(&create_composition_image().await),
  ));
}

#[rocket::get("/comic/inkplate")]
async fn comic_inkplate() -> Option<Vec<u8>> {
  return Some(inkplate_raw_image_data(&create_composition_image().await));
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
      rocket::routes![comic_color, comic_grayscale, comic_inkplate],
    )
    .launch()
    .await
    .unwrap();
}
