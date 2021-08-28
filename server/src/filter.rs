use std::sync::Arc;

use image::DynamicImage;

pub trait Filter {
  fn is_valid(&self, image: Arc<DynamicImage>) -> bool;
}

pub enum ImageFilter {
  TensorFlow(TensorFlowFilter),
}

impl Filter for ImageFilter {
  fn is_valid(&self, image: Arc<DynamicImage>) -> bool {
    match self {
      ImageFilter::TensorFlow(ref filter) => filter.is_valid(image),
    }
  }
}

impl From<TensorFlowFilter> for ImageFilter {
  fn from(filter: TensorFlowFilter) -> Self {
    Self::TensorFlow(filter)
  }
}

pub struct TensorFlowFilter {}

impl TensorFlowFilter {
  pub fn new() -> Self {
    TensorFlowFilter {}
  }
}

impl Filter for TensorFlowFilter {
  fn is_valid(&self, _image: Arc<DynamicImage>) -> bool {
    true
  }
}
