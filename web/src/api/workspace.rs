use dioxus::prelude::*;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub name: String,
    pub size: u64,
    pub modified: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub root: String,
    pub files: Vec<WorkspaceFile>,
}

#[server]
pub async fn list_workspace() -> ServerFnResult<Workspace> {
    use crate::server::workspace;

    Ok(Workspace {
        root: workspace::root_display(),
        files: workspace::list_files(),
    })
}
