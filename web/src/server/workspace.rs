//! The workspace directory: where nodeset files live, and what is in it.

use std::path::{
    Path,
    PathBuf,
};
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use uanedit::types::DateTime;

use crate::api::WorkspaceFile;
use crate::server::ServerError;

const DEFAULT_ROOT: &str = "./workspace";
/// `dx serve` runs the server with `web/` as its working directory, so the repository's own
/// workspace directory is one level up from it.
const PARENT_ROOT: &str = "../workspace";

pub fn root() -> PathBuf {
    if let Some(configured) = std::env::var_os("UANEDIT_WORKSPACE") {
        return PathBuf::from(configured);
    }
    let here = PathBuf::from(DEFAULT_ROOT);
    if here.is_dir() {
        return here;
    }
    let above = PathBuf::from(PARENT_ROOT);
    if above.is_dir() { above } else { here }
}

/// An absolute path even when the directory does not exist, so the empty state can name it.
pub fn root_display() -> String {
    let root = root();
    let absolute = std::fs::canonicalize(&root).unwrap_or_else(|_| match std::env::current_dir() {
        Ok(current) => current.join(&root),
        Err(_) => root.clone(),
    });
    absolute
        .components()
        .collect::<PathBuf>()
        .display()
        .to_string()
}

/// A name the client sent, accepted only if it names a file directly inside the workspace.
pub fn file_name(name: &str) -> Result<String, ServerError> {
    let bare = Path::new(name).file_name().is_some_and(|only| only == name);
    match bare && name != "." && name != ".." {
        true => Ok(name.to_owned()),
        false => Err(ServerError::BadName(name.to_owned())),
    }
}

pub fn path(name: &str) -> Result<PathBuf, ServerError> {
    Ok(root().join(file_name(name)?))
}

pub fn read(path: &Path) -> Result<String, ServerError> {
    std::fs::read_to_string(path).map_err(|source| ServerError::Read {
        path: path.display().to_string(),
        source,
    })
}

pub fn write(
    path: &Path,
    contents: &str,
) -> Result<(), ServerError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ServerError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(path, contents).map_err(|source| ServerError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// Every `.xml` file in the workspace, by name. A missing directory is an empty workspace.
pub fn list_files() -> Vec<WorkspaceFile> {
    let Ok(entries) = std::fs::read_dir(root()) else {
        return Vec::new();
    };

    let mut files: Vec<WorkspaceFile> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && has_xml_extension(path))
        .map(|path| describe(&path))
        .collect();

    files.sort_by_key(|file| file.name.to_lowercase());
    files
}

fn has_xml_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
}

pub fn describe(path: &Path) -> WorkspaceFile {
    let metadata = path.metadata().ok();
    WorkspaceFile {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        size: metadata.as_ref().map_or(0, std::fs::Metadata::len),
        modified: metadata
            .and_then(|metadata| metadata.modified().ok())
            .map(to_iso8601),
    }
}

/// UTC `YYYY-MM-DDTHH:MM:SSZ`, so the browser half needs no date crate.
fn to_iso8601(time: SystemTime) -> String {
    let seconds = unix_seconds(time);
    let days = seconds.div_euclid(86_400);
    let time_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time_of_day / 3600,
        (time_of_day / 60) % 60,
        time_of_day % 60
    )
}

/// Days since the Unix epoch to a proleptic Gregorian date (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;

    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    (year, month, day)
}

/// The current instant, which is what a `Model`'s PublicationDate wants. The clock is here because
/// the domain crate has none.
pub fn now() -> DateTime {
    DateTime::from_unix_seconds(unix_seconds(SystemTime::now()))
}

fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or_default()
}
