mod composition;

use chrono::DateTime;
use composition::create_composition_image;
use egg_mode::tweet::Timeline;
use egg_mode::user::UserID;
use egg_mode::Token;
use image::DynamicImage;
use rand::seq::SliceRandom;
use rocket::http::ContentType;
use rocket::response::content;
use serde::Deserialize;
use std::cmp::max;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

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

fn access_token() -> Token {
    let api_token = egg_mode::KeyPair::new(
        CONFIG.get().consumer_key.clone(),
        CONFIG.get().consumer_secret.clone(),
    );
    let access_token = egg_mode::KeyPair::new(
        CONFIG.get().access_token.clone(),
        CONFIG.get().access_token_secret.clone(),
    );

    egg_mode::Token::Access {
        consumer: api_token,
        access: access_token,
    }
}

fn user_timeline(user_id: UserID) -> Timeline {
    egg_mode::tweet::user_timeline(user_id, false, false, &TOKEN.get())
}

#[derive(Debug, Clone)]
struct Comic {
    pub url: String,
    image: state::Storage<Arc<DynamicImage>>,
}

impl Comic {
    pub fn new(url: String) -> Self {
        Comic {
            url,
            image: state::Storage::new(),
        }
    }

    pub async fn image(&self) -> Arc<DynamicImage> {
        if None == self.image.try_get() {
            // This could essentially happen multiple times in parallel while
            // loading is in progress. As it only would cause more than needed
            // fetching and processing we ignore this case here.
            // FIXME: Maybe use an RWLock?
            println!("Fetching image: {}", self.url);
            let response = reqwest::get(self.url.as_str()).await.unwrap();
            let in_bytes = response.bytes().await.unwrap();
            let img = image::io::Reader::new(Cursor::new(in_bytes))
                .with_guessed_format()
                .unwrap()
                .decode()
                .unwrap();

            self.image.set(Arc::new(img));
        }

        self.image.get().clone()
    }
}

#[derive(Debug, Clone)]
struct ComicStrip {
    id: u64,
    comics: Vec<Comic>,
    created_at: DateTime<chrono::Utc>,
}

#[derive(Clone)]
struct UserComicCollection {
    user_id: UserID,
    comic_strips: Vec<Arc<ComicStrip>>,
    max_id: Option<u64>,
    max_amount: usize,
}

async fn refresh_user_comic_collection(collection: &UserComicCollection) -> UserComicCollection {
    let timeline = user_timeline(collection.user_id.clone())
//        .with_page_size(collection.max_amount as i32 * 3)
        .with_page_size(200)
        .older(collection.max_id);

    let (timeline, feed) = timeline.await.unwrap();

    let mut comic_strips = collection.comic_strips.clone();
    let mut ids = collection.comic_ids();

    println!("Received {} tweets: processing...", feed.len());

    for tweet in feed {
        if ids.contains(&tweet.id) {
            continue;
        }
        if let Some(media) = &tweet.entities.media {
            let mut comics: Vec<Comic> = vec![];
            for entry in media {
                comics.push(Comic::new(entry.media_url.clone()));
            }
            comic_strips.push(Arc::new(ComicStrip {
                id: tweet.id,
                created_at: tweet.created_at,
                comics,
            }));

            ids.push(tweet.id);
        }
    }

    let new_max_id = match timeline.max_id {
        Some(max_id) => Some(max_id),
        None => collection.max_id,
    };

    apply_collection_constraints(UserComicCollection {
        user_id: collection.user_id.clone(),
        max_id: new_max_id,
        max_amount: collection.max_amount,
        comic_strips,
    })
}

fn apply_collection_constraints(mut collection: UserComicCollection) -> UserComicCollection {
    collection
        .comic_strips
        .sort_by_key(|comic| comic.created_at);
    collection.comic_strips = collection
        .comic_strips
        .into_iter()
        .rev()
        .take(collection.max_amount)
        .collect();

    collection
}

impl UserComicCollection {
    fn new(user_id: UserID) -> Self {
        Self::new_with_max_amount(user_id, 100)
    }

    fn new_with_max_amount(user_id: UserID, max_amount: usize) -> Self {
        UserComicCollection {
            user_id,
            max_amount,
            max_id: None,
            comic_strips: vec![],
        }
    }

    fn comic_ids(&self) -> Vec<u64> {
        self.comic_strips.iter().map(|comic| comic.id).collect()
    }
}

async fn twitter_refresh_task(collections: Arc<Vec<Mutex<UserComicCollection>>>) {
    loop {
        println!("Refreshing...");

        for collection_mut in (*collections).iter() {
            let mut collection = collection_mut.lock().await;
            println!("Loading images from: {:?}", collection.user_id);
            *collection = refresh_user_comic_collection(&collection).await;
            for comic_strip in collection.comic_strips.iter() {
                for comic in comic_strip.comics.iter() {
                    println!(" -> {}", comic.url);
                }
            }
        }

        sleep(Duration::from_secs(CONFIG.get().twitter_refresh_interval)).await;
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
