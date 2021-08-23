use chrono::DateTime;
use egg_mode::tweet::Timeline;
use egg_mode::user::UserID;
use egg_mode::Token;
use image::DynamicImage;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::{CONFIG, TOKEN};

pub fn access_token() -> Token {
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
pub struct Comic {
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
pub struct ComicStrip {
  pub id: u64,
  pub comics: Vec<Comic>,
  pub created_at: DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct UserComicCollection {
  pub user_id: UserID,
  pub comic_strips: Vec<Arc<ComicStrip>>,
  max_id: Option<u64>,
  pub max_amount: usize,
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
        let url = entry.media_url.clone();
        println!(" -> {}", url);
        comics.push(Comic::new(url));
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
  pub fn new(user_id: UserID) -> Self {
    Self::new_with_max_amount(user_id, 100)
  }

  pub fn new_with_max_amount(user_id: UserID, max_amount: usize) -> Self {
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

pub async fn twitter_refresh_task(collections: Arc<Vec<Mutex<UserComicCollection>>>) {
  loop {
    println!("Refreshing...");

    for collection_mut in (*collections).iter() {
      let mut collection = collection_mut.lock().await;
      println!("Loading images for: {:?}", collection.user_id);
      *collection = refresh_user_comic_collection(&collection).await;
    }

    sleep(Duration::from_secs(CONFIG.get().twitter_refresh_interval)).await;
  }
}
