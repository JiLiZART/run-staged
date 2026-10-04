//! Live, animated terminal panel for run-staged: a pure [`model`], a ratatui
//! [`render`]er, and the terminal lifecycle in [`term`].

pub mod model;
pub mod render;
pub mod term;

pub use model::{Event, PlanGroup, PlanInfo, StepKind};
pub use render::Theme;
pub use term::{LiveView, terminal_width};
