//! UI shared across views.

mod app_shell;
mod create_file;
mod dialog;
mod icon;
mod pane;
mod theme;

pub use app_shell::AppShell;
pub use create_file::CreateFileDialog;
pub use dialog::{
    Dialog,
    DocumentStyles,
};
pub use icon::Icon;
#[allow(unused_imports, reason = "the placeholder pane outlives the views that used it")]
pub use pane::Pane;
pub use theme::ThemeToggle;
