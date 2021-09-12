use async_trait::async_trait;
use image::DynamicImage;
use serde::Deserialize;
use std::sync::Arc;

use crate::image_data;

#[async_trait]
pub trait Filter {
  async fn is_valid(&self, image: Arc<DynamicImage>) -> bool;
}

pub enum ImageFilter {
  HttpClassifier(HttpClassifierFilter),
}

#[async_trait]
impl Filter for ImageFilter {
  async fn is_valid(&self, image: Arc<DynamicImage>) -> bool {
    match self {
      ImageFilter::HttpClassifier(ref filter) => filter.is_valid(image).await,
    }
  }
}

impl From<HttpClassifierFilter> for ImageFilter {
  fn from(filter: HttpClassifierFilter) -> Self {
    Self::HttpClassifier(filter)
  }
}

pub struct HttpClassifierFilter {
  url: String,
}

impl HttpClassifierFilter {
  pub fn new(url: String) -> Self {
    HttpClassifierFilter { url }
  }
}

#[derive(Deserialize)]
struct Classification {
  probability: f64,
  label: String,
}

#[async_trait]
impl Filter for HttpClassifierFilter {
  async fn is_valid(&self, image: Arc<DynamicImage>) -> bool {
    let client = reqwest::Client::new();
    let request = client.post(self.url.as_str()).body(image_data::png(&image));
    if let Ok(response) = request.send().await {
      if let Ok(classification) = response.json::<Classification>().await {
        println!(
          "    ? Classification: {} with {} probability",
          classification.label, classification.probability
        );
        if classification.label == "comic" {
          println!("    ? true");
          return true;
        }
      }
    }
    println!("    ? false");
    return false;
  }
}
