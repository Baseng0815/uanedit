use dioxus::prelude::*;

use crate::components::AppShell;
use crate::views::{
    Editor,
    Files,
};

#[derive(Routable, Clone, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Files {},
    #[route("/edit/:file")]
    Editor { file: String },
}
