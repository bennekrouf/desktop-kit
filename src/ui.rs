//! What every app shows the same way. The bar across the top: the name and
//! version on the left, what the app switches between in the middle, and on
//! the right the app's own buttons, then the model, Settings, MCP, Feedback,
//! the theme and, in the corner, the licence. The banners under it: a newer
//! release, a notice, the usage-statistics question. And the Pro window.
//!
//! Its colours come from the app: each stylesheet sets the `--kit-*`
//! variables on `body` (so `body.light` changes them too), and gives its
//! full-height layout `calc(100vh - var(--kit-header-h))` to leave room for
//! the bar.

mod banners;
mod flag;
mod header;
mod pro;
mod theme;

pub use banners::Banners;
pub use flag::RegionFlag;
pub use header::{Header, ModelBadge, ProBadge, ProLook};
pub use pro::{use_pro, Pro, ProButton, ProWindow};
pub use theme::{use_theme, Theme, ThemeToggle};

/// The header's stylesheet. [`Header`] adds it itself.
pub const STYLE: &str = include_str!("ui/header.css");

#[cfg(test)]
mod tests;
