//! uanedit's web app: the browser client and the server that owns the workspace directory.

mod api;
mod components;
mod export;
mod route;
mod session;
mod views;

#[cfg(feature = "server")]
mod server;

use dioxus::prelude::*;

use crate::route::Route;

const MAIN_CSS: Asset = asset!("/assets/main.css");
const FAVICON: Asset = asset!("/assets/favicon.svg");

// A folder asset, because manganis does not rewrite `url()` in CSS: the stylesheet's `@font-face`
// rules resolve against its own directory, and only a folder asset keeps the file names. `#[used]`
// because nothing in Rust reads it — the linker would otherwise drop it.
#[used]
static FONTS: Asset = asset!("/assets/fonts", AssetOptions::folder());

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}
