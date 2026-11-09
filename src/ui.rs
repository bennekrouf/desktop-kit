//! What every app shows the same way. The bar across the top: the model's
//! flag, the Settings gear, MCP, Feedback and, in the corner, the licence;
//! under it, a toolbar with what the app switches between. Settings starts
//! with the theme and the version. The banners under it: a newer
//! release, a notice, the usage-statistics question. The Pro window. And the
//! media library and pipeline calendar the apps share, and the file picker
//! they open instead of Finder or Explorer.
//!
//! Its colours come from the app: each stylesheet sets the `--kit-*`
//! variables on `body` (so `body.light` changes them too), and gives its
//! full-height layout `calc(100vh - var(--kit-header-h))` to leave room for
//! the bar.

mod banners;
mod connect;
mod escape;
mod flag;
mod header;
mod library;
mod picker;
mod pro;
mod settings;
mod theme;

pub use banners::Banners;
pub use connect::ConnectWindow;
pub use escape::use_escape;
pub use flag::{use_closed_regions, RegionFlag};
pub use header::{Header, ModelBadge, ProBadge, ProLook, Toolbar};
pub use library::{open_in, use_library, DragGhost, LibraryView, PipelineCalendar, LIBRARY_STYLE};
pub use picker::{FilePicker, PICKER_STYLE};
pub use pro::{use_pro, Pro, ProButton, ProWindow};
pub use settings::{SettingsGeneral, SettingsWindow};
pub use theme::{use_theme, Theme, ThemeToggle};

/// The header's stylesheet. [`Header`] adds it itself.
pub const STYLE: &str = include_str!("ui/header.css");

#[cfg(test)]
mod tests;
