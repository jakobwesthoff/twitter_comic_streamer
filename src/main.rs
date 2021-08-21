use chrono::DateTime;
use egg_mode::tweet::Timeline;
use egg_mode::user::UserID;
use egg_mode::Token;
use rand::seq::SliceRandom;
use rocket::http::ContentType;
use rocket::response::content;
use serde::Deserialize;
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
    id: u64,
    urls: Vec<String>,
    created_at: DateTime<chrono::Utc>,
}

#[derive(Clone)]
struct UserComicCollection {
    user_id: UserID,
    comics: Vec<Comic>,
    max_id: Option<u64>,
    max_amount: usize,
}

async fn refresh_user_comic_collection(collection: &UserComicCollection) -> UserComicCollection {
    let timeline = user_timeline(collection.user_id.clone())
        .with_page_size(collection.max_amount as i32 * 3)
        .older(collection.max_id);

    let (timeline, feed) = timeline.await.unwrap();

    let mut comics: Vec<Comic> = collection.comics.clone();
    let mut ids = collection.comic_ids();

    println!("Received {} tweets: processing...", feed.len());

    for tweet in feed {
        if ids.contains(&tweet.id) {
            continue;
        }
        if let Some(media) = &tweet.entities.media {
            let mut urls: Vec<String> = vec![];
            for entry in media {
                urls.push(entry.media_url.clone());
            }
            comics.push(Comic {
                id: tweet.id,
                created_at: tweet.created_at,
                urls,
            });

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
        comics,
    })
}

fn apply_collection_constraints(mut collection: UserComicCollection) -> UserComicCollection {
    collection.comics.sort_by_key(|comic| comic.created_at);
    collection.comics = collection
        .comics
        .into_iter()
        .rev()
        .take(collection.max_amount)
        .collect();

    collection
}

impl UserComicCollection {
    fn new(user_id: UserID) -> Self {
        Self::new_with_max_amount(user_id, 10)
    }

    fn new_with_max_amount(user_id: UserID, max_amount: usize) -> Self {
        UserComicCollection {
            user_id,
            max_amount,
            max_id: None,
            comics: vec![],
        }
    }

    fn comic_ids(&self) -> Vec<u64> {
        self.comics.iter().map(|comic| comic.id).collect()
    }
}

async fn twitter_refresh_task(collections: Arc<Vec<Mutex<UserComicCollection>>>) {
    loop {
        println!("Refreshing...");

        for collection_mut in (*collections).iter() {
            let mut collection = collection_mut.lock().await;
            println!("Loading images from: {:?}", collection.user_id);
            *collection = refresh_user_comic_collection(&collection).await;
            for comic in collection.comics.iter() {
                for url in comic.urls.iter() {
                    println!(" -> {}", url);
                }
            }
        }

        sleep(Duration::from_secs(CONFIG.get().twitter_refresh_interval)).await;
    }
}

async fn random_comic() -> Option<Comic> {
    let collections = COLLECTION_ARC.get();
    let mut collection_refs: Vec<&Mutex<UserComicCollection>> = (*collections).iter().collect();
    collection_refs.shuffle(&mut rand::thread_rng());

    for shuffled_ref in collection_refs {
        let locked_collection = shuffled_ref.lock().await;
        if locked_collection.comics.len() > 0 {
            return Some(
                locked_collection
                    .comics
                    .choose(&mut rand::thread_rng())
                    .unwrap()
                    .clone(),
            );
        }
    }

    return None;
}

async fn image_data(url: &String) -> Vec<u8> {
    let response = reqwest::get(url).await.unwrap();
    let in_bytes = response.bytes().await.unwrap();
    let img = image::io::Reader::new(Cursor::new(in_bytes))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();

    let mut out_bytes: Vec<u8> = Vec::new();
    img.write_to(&mut out_bytes, image::ImageOutputFormat::Png)
        .unwrap();

    out_bytes
}

#[rocket::get("/comic")]
async fn comic() -> Option<content::Custom<Vec<u8>>> {
    if let Some(comic) = random_comic().await {
        return Some(content::Custom(
            ContentType::PNG,
            image_data(&comic.urls[0]).await,
        ));
    }

    return None;
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
        .mount("/", rocket::routes![comic])
        .launch()
        .await
        .unwrap();
}
