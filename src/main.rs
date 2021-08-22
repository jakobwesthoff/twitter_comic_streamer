mod composition;
mod twitter;

use twitter::{access_token, twitter_refresh_task, UserComicCollection};
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

async fn png_image_data(image: &DynamicImage) -> Vec<u8> {
    let mut out_bytes: Vec<u8> = Vec::new();
    image
        .write_to(&mut out_bytes, image::ImageOutputFormat::Png)
        .unwrap();

    out_bytes
}

#[rocket::get("/comic")]
async fn comic() -> Option<content::Custom<Vec<u8>>> {
    let comic_strips = random_comic_strips().await;
    if comic_strips.len() > 0 {
        return Some(content::Custom(
            ContentType::PNG,
            png_image_data(&*comic_strips[0].comics[0].image().await).await,
        ));
    }

    return None;
}

#[rocket::get("/composition")]
async fn comic_composition() -> Option<content::Custom<Vec<u8>>> {
    return Some(content::Custom(
        ContentType::PNG,
        png_image_data(&create_composition_image().await).await,
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
        .mount("/", rocket::routes![comic, comic_composition])
        .launch()
        .await
        .unwrap();
}
