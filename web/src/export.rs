//! What the export dialog asks for, and how the browser is handed the result.
//!
//! The XML comes from the server, because only the server holds the bytes the codec splices into;
//! the C pair and the Rust wrapper are compiled here, because `uanedit::compile` sits outside the
//! `xml` feature. Both meet in one list of files, delivered as separate downloads or as one archive.

use std::io::{
    Cursor,
    Write,
};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// The dialog's answer: what to put in the export, and how to hand it over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportRequest {
    pub xml: bool,
    pub c_sources: bool,
    /// Only reached when `c_sources` is set — the wrapper has nothing to wrap without the pair.
    pub rust_bindings: bool,
    pub archive: bool,
}

impl Default for ExportRequest {
    /// The XML alone, as its own file: compiling the address space costs a full walk, so an export
    /// that was not asked for code does not do one, and a single file has nothing to archive.
    fn default() -> Self {
        Self {
            xml: true,
            c_sources: false,
            rust_bindings: true,
            archive: false,
        }
    }
}

impl ExportRequest {
    pub fn is_empty(self) -> bool {
        !self.xml && !self.c_sources
    }

    /// The names the export produces, for the dialog to show before it runs.
    pub fn file_names(
        self,
        file_name: &str,
    ) -> Vec<String> {
        let base = uanedit::compile::base_name(file_name);
        let mut names = Vec::new();
        if self.xml {
            names.push(file_name.to_owned());
        }
        if self.c_sources {
            names.push(format!("{base}.c"));
            names.push(format!("{base}.h"));
            if self.rust_bindings {
                names.push(format!("{base}.rs"));
            }
        }
        names
    }
}

pub fn archive_name(file_name: &str) -> String {
    format!("{}.zip", uanedit::compile::base_name(file_name))
}

/// One export's output: the files in the order the dialog lists them, and what the compile
/// counted, kept after its strings moved into that list.
#[derive(Default)]
pub struct Bundle {
    pub files: Vec<(String, String)>,
    pub compiled: Option<CompileCounts>,
}

#[derive(Clone, Copy)]
pub struct CompileCounts {
    pub nodes: usize,
    pub skipped: usize,
}

/// Clicks a blob URL per file, or one for the archive holding them all.
pub fn deliver(
    files: Vec<(String, String)>,
    archive: Option<String>,
) -> Result<(), String> {
    match archive {
        Some(name) => {
            let bytes = zip(&files)?;
            send(ARCHIVE_JS, (name, STANDARD.encode(bytes)))
        }
        None => send(DOWNLOAD_JS, files),
    }
}

/// Puts the files behind blob URLs and gives the URLs back, so the dialog can offer them as links
/// that open in a tab. Handing back a URL rather than opening it keeps the click a user gesture,
/// which is what a popup blocker looks for.
pub async fn link(files: &[(String, String)]) -> Result<Vec<String>, String> {
    let mut eval = dioxus::prelude::document::eval(LINK_JS);
    eval.send(files).map_err(|error| error.to_string())?;
    eval.recv::<Vec<String>>()
        .await
        .map_err(|error| error.to_string())
}

/// Releases URLs a newer batch replaced, which a page that never reloads would otherwise hold.
pub fn revoke(urls: Vec<String>) {
    if !urls.is_empty() {
        let _ = send(REVOKE_JS, urls);
    }
}

fn send(
    script: &str,
    payload: impl serde::Serialize,
) -> Result<(), String> {
    dioxus::prelude::document::eval(script)
        .send(payload)
        .map_err(|error| error.to_string())
}

fn zip(files: &[(String, String)]) -> Result<Vec<u8>, String> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, text) in files {
        writer
            .start_file(name.as_str(), options)
            .and_then(|()| writer.write_all(text.as_bytes()).map_err(Into::into))
            .map_err(|error| format!("{name}: {error}"))?;
    }
    Ok(writer
        .finish()
        .map_err(|error| error.to_string())?
        .into_inner())
}

/// Receives a list of `(name, text)` pairs; the browser asks once to allow multiple downloads.
const DOWNLOAD_JS: &str = r#"
    const files = await dioxus.recv();
    for (const [name, text] of files) {
        const url = URL.createObjectURL(new Blob([text], { type: "application/octet-stream" }));
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = name;
        anchor.click();
        setTimeout(() => URL.revokeObjectURL(url), 60000);
    }
"#;

/// Receives a list of `(name, text)` pairs and answers with one blob URL each, in the same order.
/// `text/plain` so a browser renders the file rather than offering to save it.
const LINK_JS: &str = r#"
    const files = await dioxus.recv();
    dioxus.send(files.map(([name, text]) =>
        URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }))));
"#;

const REVOKE_JS: &str = r#"
    const urls = await dioxus.recv();
    for (const url of urls) {
        URL.revokeObjectURL(url);
    }
"#;

/// Receives `(name, base64)`, because the eval channel is JSON and bytes have to survive it.
const ARCHIVE_JS: &str = r#"
    const [name, encoded] = await dioxus.recv();
    const binary = atob(encoded);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
        bytes[index] = binary.charCodeAt(index);
    }
    const url = URL.createObjectURL(new Blob([bytes], { type: "application/zip" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
"#;
