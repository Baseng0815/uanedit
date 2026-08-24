use dioxus::prelude::*;
use uanedit::types::DateTime;

/// The server's clock, which is the one a PublicationDate is stamped from — the domain crate has
/// none and the browser's may be anything.
#[server]
pub async fn current_time() -> ServerFnResult<DateTime> {
    use crate::server::workspace;

    Ok(workspace::now())
}
