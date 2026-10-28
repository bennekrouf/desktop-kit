//! The bar across the top of every app, so they all look and read the same:
//! the name and version on the left, what the app switches between in the
//! middle, and on the right the app's own buttons, then the model, Settings,
//! MCP, Feedback, the theme and, in the corner, the licence.
//!
//! Its colours come from the app: each stylesheet sets the `--kit-*`
//! variables on `body` (so `body.light` changes them too), and gives its
//! full-height layout `calc(100vh - var(--kit-header-h))` to leave room for
//! the bar.

mod flag;
mod header;
mod theme;

pub use flag::RegionFlag;
pub use header::{Header, ModelBadge, ProBadge, ProLook};
pub use theme::{use_theme, Theme, ThemeToggle};

/// The header's stylesheet. [`Header`] adds it itself.
pub const STYLE: &str = include_str!("ui/header.css");

#[cfg(test)]
mod tests;
