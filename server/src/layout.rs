use std::sync::Arc;

use cassowary::strength::{MEDIUM, REQUIRED, STRONG};
use cassowary::WeightedRelation::*;
use cassowary::{Solver, Variable};
use image::{DynamicImage, GenericImageView};

const MAX_WIDTH: f64 = 1200.0;
const MAX_HEIGHT: f64 = 825.0;

#[inline(always)]
fn aspect_ratio(image: &Arc<DynamicImage>) -> f64 {
  let (width, height) = image.dimensions();
  width as f64 / height as f64
}

pub struct ConstrainedBox<'a> {
  solver: Option<&'a Solver>,
  x: Variable,
  y: Variable,
  w: Variable,
  h: Variable,
}

impl<'a> ConstrainedBox<'a> {
  pub fn new() -> Self {
    ConstrainedBox {
      solver: None,
      x: Variable::new(),
      y: Variable::new(),
      w: Variable::new(),
      h: Variable::new(),
    }
  }

  pub fn x(&self) -> f64 {
    self.solver.unwrap().get_value(self.x)
  }

  pub fn y(&self) -> f64 {
    self.solver.unwrap().get_value(self.y)
  }

  pub fn w(&self) -> f64 {
    self.solver.unwrap().get_value(self.w)
  }

  pub fn h(&self) -> f64 {
    self.solver.unwrap().get_value(self.h)
  }

  pub fn set_solver(&mut self, solver: &'a Solver) {
    self.solver = Some(solver);
  }
}

pub struct DrawingInstruction {
  pub image: Arc<DynamicImage>,
  pub x: u32,
  pub y: u32,
  pub w: u32,
  pub h: u32,
}

impl DrawingInstruction {
  pub fn new(image: Arc<DynamicImage>, constrained_box: &ConstrainedBox) -> Self {
    DrawingInstruction {
      image,
      x: constrained_box.x().floor() as u32,
      y: constrained_box.y().floor() as u32,
      w: constrained_box.w().floor() as u32,
      h: constrained_box.h().floor() as u32,
    }
  }
}

pub trait CalculateLayout {
  fn calculate(&self) -> Vec<DrawingInstruction>;
}

pub struct SingleLayout {
  margin: f64,
  primary: Arc<DynamicImage>,
}

pub struct ColumnLayout {
  margin: f64,
  primary: Arc<DynamicImage>,
  secondary: Vec<Arc<DynamicImage>>,
}

pub struct RowLayout {
  margin: f64,
  primary: Arc<DynamicImage>,
  secondary: Vec<Arc<DynamicImage>>,
}

pub enum Layout {
  Single(SingleLayout),
  Column(ColumnLayout),
  Row(RowLayout),
}

impl SingleLayout {
  #[allow(dead_code)]
  pub fn new(primary: Arc<DynamicImage>) -> Self {
    Self::new_with_margin(primary, 0.0)
  }

  #[allow(dead_code)]
  pub fn new_with_margin(primary: Arc<DynamicImage>, margin: f64) -> Self {
    SingleLayout { primary, margin }
  }
}

impl CalculateLayout for SingleLayout {
  fn calculate(&self) -> Vec<DrawingInstruction> {
    let mut instructions = vec![];

    let mut solver = Solver::new();
    let mut pb = ConstrainedBox::new();

    solver
      .add_constraints(&[
        /* Center within given space */
        pb.y | EQ(REQUIRED) | MAX_HEIGHT - (pb.y + pb.h),
        pb.x | EQ(REQUIRED) | MAX_WIDTH - (pb.x + pb.w),
        /* Always keep aspect ratio */
        pb.w | EQ(REQUIRED) | pb.h * aspect_ratio(&self.primary),
        /* Always apply margin to all four sides */
        pb.x | GE(REQUIRED) | self.margin,
        pb.y | GE(REQUIRED) | self.margin,
        pb.x + pb.w | LE(REQUIRED) | MAX_WIDTH - self.margin,
        pb.y + pb.h | LE(REQUIRED) | MAX_WIDTH - self.margin,
        /* Either height or width should be maximized */
        pb.w | EQ(STRONG) | MAX_WIDTH - self.margin * 2.0,
        pb.h | EQ(STRONG) | MAX_HEIGHT - self.margin * 2.0,
      ])
      .unwrap();

    pb.set_solver(&solver);
    println!(
      "SingleLayout Primary: x: {:?}, y: {:?}, w: {:?}, h: {:?}",
      pb.x(),
      pb.y(),
      pb.w(),
      pb.h(),
    );

    instructions.push(DrawingInstruction::new(self.primary.clone(), &pb));

    instructions
  }
}

impl ColumnLayout {
  #[allow(dead_code)]
  pub fn new(primary: Arc<DynamicImage>, secondary: Vec<Arc<DynamicImage>>) -> Self {
    ColumnLayout::new_with_margin(primary, secondary, 0.0)
  }

  #[allow(dead_code)]
  pub fn new_with_margin(
    primary: Arc<DynamicImage>,
    secondary: Vec<Arc<DynamicImage>>,
    margin: f64,
  ) -> Self {
    ColumnLayout {
      primary,
      secondary,
      margin,
    }
  }
}

impl CalculateLayout for ColumnLayout {
  fn calculate(&self) -> Vec<DrawingInstruction> {
    let mut instructions = vec![];

    let mut solver = Solver::new();
    let mut primary_box = ConstrainedBox::new();
    let mut secondary_boxes: Vec<ConstrainedBox> = self
      .secondary
      .iter()
      .map(|_| ConstrainedBox::new())
      .collect();

    // Ensure we are never starting negative
    solver
      .add_constraints(&[
        primary_box.x | GE(REQUIRED) | 0.0,
        primary_box.y | GE(REQUIRED) | 0.0,
      ])
      .unwrap();

    // Keep aspect ratio for all boxes
    solver
      .add_constraint(primary_box.w | EQ(REQUIRED) | primary_box.h * aspect_ratio(&self.primary))
      .unwrap();
    for (index, secondary_box) in secondary_boxes.iter().enumerate() {
      solver
        .add_constraint(
          secondary_box.w | EQ(REQUIRED) | secondary_box.h * aspect_ratio(&self.secondary[index]),
        )
        .unwrap();
    }

    // Center primary vertically
    solver
      .add_constraint(primary_box.y | EQ(REQUIRED) | MAX_HEIGHT - (primary_box.y + primary_box.h))
      .unwrap();

    // Margin for primary
    solver
      .add_constraints(&[
        primary_box.x | GE(REQUIRED) | self.margin,
        primary_box.y | GE(MEDIUM) | self.margin,
      ])
      .unwrap();
    // Vertical start of primary and secondary is identical
    let first_secondary = secondary_boxes.first().unwrap();
    solver
      .add_constraint(primary_box.y | EQ(REQUIRED) | first_secondary.y)
      .unwrap();

    // Vertical end of primary and secondary is identical
    let last_secondary = secondary_boxes.last().unwrap();
    solver
      .add_constraint(
        primary_box.y + primary_box.h | EQ(REQUIRED) | last_secondary.y + last_secondary.h,
      )
      .unwrap();

    // Horizontal margin between primary and secondary is at least margin
    for secondary_box in secondary_boxes.iter() {
      solver
        .add_constraint(
          secondary_box.x | EQ(REQUIRED) | primary_box.x + primary_box.w + self.margin,
        )
        .unwrap();
    }

    // Center primary and secondaries horizontally
    solver
      .add_constraint(
        primary_box.x | EQ(REQUIRED) | MAX_WIDTH - (first_secondary.x + first_secondary.w),
      )
      .unwrap();

    // Vertical margin between secondaries
    for index in 1..secondary_boxes.len() {
      let current_secondary = &secondary_boxes[index];
      let previous_secondary = &secondary_boxes[index - 1];

      solver
        .add_constraint(
          current_secondary.y
            | EQ(REQUIRED)
            | previous_secondary.y + previous_secondary.h + self.margin,
        )
        .unwrap();
    }

    // All secondaries start at the same x
    for index in 1..secondary_boxes.len() {
      let current_secondary = &secondary_boxes[index];
      let previous_secondary = &secondary_boxes[index - 1];

      solver
        .add_constraint(current_secondary.x | EQ(REQUIRED) | previous_secondary.x)
        .unwrap();
    }

    // All secondary images are aligned with the right border
    for secondary_box in secondary_boxes.iter() {
      solver
        .add_constraint(secondary_box.x + secondary_box.w | LE(REQUIRED) | MAX_WIDTH - self.margin)
        .unwrap();
    }

    primary_box.set_solver(&solver);
    println!(
      "ColumnLayout Primary: x: {:?}, y: {:?}, w: {:?}, h: {:?}",
      primary_box.x(),
      primary_box.y(),
      primary_box.w(),
      primary_box.h(),
    );

    for secondary_box in secondary_boxes.iter_mut() {
      secondary_box.set_solver(&solver);
      println!(
        "ColumnLayout Secondary: x: {:?}, y: {:?}, w: {:?}, h: {:?}",
        secondary_box.x(),
        secondary_box.y(),
        secondary_box.w(),
        secondary_box.h(),
      );
    }

    instructions.push(DrawingInstruction::new(self.primary.clone(), &primary_box));

    for (index, secondary_box) in secondary_boxes.iter().enumerate() {
      instructions.push(DrawingInstruction::new(
        self.secondary[index].clone(),
        &secondary_box,
      ));
    }

    instructions
  }
}

impl RowLayout {
  #[allow(dead_code)]
  pub fn new(primary: Arc<DynamicImage>, secondary: Vec<Arc<DynamicImage>>) -> Self {
    Self::new_with_margin(primary, secondary, 0.0)
  }

  #[allow(dead_code)]
  pub fn new_with_margin(
    primary: Arc<DynamicImage>,
    secondary: Vec<Arc<DynamicImage>>,
    margin: f64,
  ) -> Self {
    Self {
      primary,
      secondary,
      margin,
    }
  }
}
impl CalculateLayout for RowLayout {
  fn calculate(&self) -> Vec<DrawingInstruction> {
    let mut instructions = vec![];

    let mut solver = Solver::new();
    let mut primary_box = ConstrainedBox::new();
    let mut secondary_boxes: Vec<ConstrainedBox> = self
      .secondary
      .iter()
      .map(|_| ConstrainedBox::new())
      .collect();

    // FIXME: Extract
    // Ensure we are never starting negative
    solver
      .add_constraints(&[
        primary_box.x | GE(REQUIRED) | 0.0,
        primary_box.y | GE(REQUIRED) | 0.0,
      ])
      .unwrap();

    // FIXME: Extract
    // Keep aspect ratio for all boxes
    solver
      .add_constraint(primary_box.w | EQ(REQUIRED) | primary_box.h * aspect_ratio(&self.primary))
      .unwrap();
    for (index, secondary_box) in secondary_boxes.iter().enumerate() {
      solver
        .add_constraint(
          secondary_box.w | EQ(REQUIRED) | secondary_box.h * aspect_ratio(&self.secondary[index]),
        )
        .unwrap();
    }

    // Center primary horizontally
    solver
      .add_constraint(primary_box.x | EQ(REQUIRED) | MAX_WIDTH - (primary_box.x + primary_box.w))
      .unwrap();

    //FIXME: Extract
    // Margin for primary
    solver
      .add_constraints(&[
        primary_box.x | GE(REQUIRED) | self.margin,
        primary_box.y | GE(MEDIUM) | self.margin,
      ])
      .unwrap();

    // Horizontal start of primary and secondary is identical
    let first_secondary = secondary_boxes.first().unwrap();
    solver
      .add_constraint(primary_box.x | EQ(REQUIRED) | first_secondary.x)
      .unwrap();

    // Horizontal end of primary and secondary is identical
    let last_secondary = secondary_boxes.last().unwrap();
    solver
      .add_constraint(
        primary_box.x + primary_box.w | EQ(REQUIRED) | last_secondary.x + last_secondary.w,
      )
      .unwrap();

    // Vertical margin between primary and secondary is at least margin
    for secondary_box in secondary_boxes.iter() {
      solver
        .add_constraint(
          secondary_box.y | EQ(REQUIRED) | primary_box.y + primary_box.h + self.margin,
        )
        .unwrap();
    }

    // Center primary and secondaries vertically
    solver
      .add_constraint(
        primary_box.y | EQ(REQUIRED) | MAX_HEIGHT - (first_secondary.y + first_secondary.h),
      )
      .unwrap();

    // Horizontal margin between secondaries
    for index in 1..secondary_boxes.len() {
      let current_secondary = &secondary_boxes[index];
      let previous_secondary = &secondary_boxes[index - 1];

      solver
        .add_constraint(
          current_secondary.x
            | EQ(REQUIRED)
            | previous_secondary.x + previous_secondary.w + self.margin,
        )
        .unwrap();
    }

    // All secondaries start at the same y
    for index in 1..secondary_boxes.len() {
      let current_secondary = &secondary_boxes[index];
      let previous_secondary = &secondary_boxes[index - 1];

      solver
        .add_constraint(current_secondary.y | EQ(REQUIRED) | previous_secondary.y)
        .unwrap();
    }

    // All secondary images are aligned with the bottom border
    for secondary_box in secondary_boxes.iter() {
      solver
        .add_constraint(secondary_box.y + secondary_box.h | LE(REQUIRED) | MAX_HEIGHT - self.margin)
        .unwrap();
    }

    primary_box.set_solver(&solver);
    println!(
      "RowLayout Primary: x: {:?}, y: {:?}, w: {:?}, h: {:?}",
      primary_box.x(),
      primary_box.y(),
      primary_box.w(),
      primary_box.h(),
    );

    for secondary_box in secondary_boxes.iter_mut() {
      secondary_box.set_solver(&solver);
      println!(
        "RowLayout Secondary: x: {:?}, y: {:?}, w: {:?}, h: {:?}",
        secondary_box.x(),
        secondary_box.y(),
        secondary_box.w(),
        secondary_box.h(),
      );
    }

    instructions.push(DrawingInstruction::new(self.primary.clone(), &primary_box));

    for (index, secondary_box) in secondary_boxes.iter().enumerate() {
      instructions.push(DrawingInstruction::new(
        self.secondary[index].clone(),
        &secondary_box,
      ));
    }

    instructions
  }
}

impl CalculateLayout for Layout {
  fn calculate(&self) -> Vec<DrawingInstruction> {
    match self {
      Layout::Single(ref single_layout) => single_layout.calculate(),
      Layout::Column(ref column_layout) => column_layout.calculate(),
      Layout::Row(ref row_layout) => row_layout.calculate(),
    }
  }
}

impl From<SingleLayout> for Layout {
  fn from(inner: SingleLayout) -> Self {
    Self::Single(inner)
  }
}

impl From<ColumnLayout> for Layout {
  fn from(inner: ColumnLayout) -> Self {
    Self::Column(inner)
  }
}

impl From<RowLayout> for Layout {
  fn from(inner: RowLayout) -> Self {
    Self::Row(inner)
  }
}
