use dioxus::prelude::*;

use crate::components::Icon;

/// The theme override: the tokens are `light-dark()` pairs on `color-scheme: light dark`, so
/// forcing a theme is one `data-theme` attribute on the root element, and following the system is
/// its absence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Stamps a stored override before first paint, so a dark-mode reload never flashes light.
const BOOT_JS: &str = "const theme = localStorage.getItem('uanedit-theme'); if (theme === 'light' || theme === 'dark') document.documentElement.dataset.theme = theme;";

const READ_JS: &str = "return localStorage.getItem('uanedit-theme');";

impl Theme {
    fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    fn from_stored(stored: &str) -> Option<Self> {
        match stored {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::System => "brightness_auto",
            Self::Light => "light_mode",
            Self::Dark => "dark_mode",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::System => "Theme: following the system (click for light)",
            Self::Light => "Theme: light (click for dark)",
            Self::Dark => "Theme: dark (click to follow the system)",
        }
    }

    fn apply_js(self) -> &'static str {
        match self {
            Self::System => "delete document.documentElement.dataset.theme; localStorage.removeItem('uanedit-theme');",
            Self::Light => {
                "document.documentElement.dataset.theme = 'light'; localStorage.setItem('uanedit-theme', 'light');"
            }
            Self::Dark => {
                "document.documentElement.dataset.theme = 'dark'; localStorage.setItem('uanedit-theme', 'dark');"
            }
        }
    }
}

#[component]
pub fn ThemeToggle() -> Element {
    let mut theme = use_signal(Theme::default);
    use_future(move || async move {
        if let Ok(stored) = document::eval(READ_JS).await
            && let Some(parsed) = stored.as_str().and_then(Theme::from_stored)
        {
            theme.set(parsed);
        }
    });
    let current = theme();

    rsx! {
        document::Script { {BOOT_JS} }
        button {
            class: "icon-button",
            title: current.title(),
            onclick: move |_| {
                let next = theme().next();
                theme.set(next);
                document::eval(next.apply_js());
            },
            Icon { name: current.icon() }
        }
    }
}
