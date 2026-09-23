//! Release tar.gz 安全解包与制品归一化。

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::{OperationError, internal, invalid};

const APP_BINARY_NAME: &str = "codex-proxy-rs";
const MAX_EXTRACTED_SIZE: u64 = 1024 * 1024 * 1024;
const MAX_ARCHIVE_FILES: usize = 20_000;

#[derive(Debug)]
pub(crate) struct ExtractedRelease {
    pub(crate) binary_path: PathBuf,
    pub(crate) web_dist_dir: Option<PathBuf>,
    pub(crate) modeltrace_dir: Option<PathBuf>,
    pub(crate) companions: Vec<PathBuf>,
}

pub(crate) fn extract_release(
    archive_path: &Path,
    temp_dir: &Path,
) -> Result<ExtractedRelease, OperationError> {
    let file = fs::File::open(archive_path)
        .map_err(|error| internal(format!("failed to open release archive: {error}")))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let binary_path = temp_dir.join(APP_BINARY_NAME);
    let web_dist_dir = temp_dir.join("web-dist");
    let modeltrace_dir = temp_dir.join("modeltrace");
    let mut found_binary = false;
    let mut found_web = false;
    let mut found_modeltrace = false;
    let mut extracted_size = 0_u64;
    let mut file_count = 0_usize;
    let mut companions = Vec::new();
    let mut destinations = HashSet::new();

    for entry in archive
        .entries()
        .map_err(|error| internal(format!("failed to read release archive: {error}")))?
    {
        let mut entry =
            entry.map_err(|error| internal(format!("invalid archive entry: {error}")))?;
        let path = entry
            .path()
            .map_err(|error| internal(format!("invalid archive path: {error}")))?
            .into_owned();
        if unsafe_archive_path(&path) {
            return Err(invalid("release archive contains an unsafe path"));
        }
        if !entry.header().entry_type().is_file() {
            if !entry.header().entry_type().is_dir() {
                return Err(invalid("release archive contains a link or special file"));
            }
            continue;
        }
        if entry.header().mode().unwrap_or(0o7000) & 0o7000 != 0 {
            return Err(invalid("release archive contains special permissions"));
        }
        file_count = file_count.saturating_add(1);
        extracted_size = extracted_size.saturating_add(entry.header().size().unwrap_or(u64::MAX));
        if file_count > MAX_ARCHIVE_FILES || extracted_size > MAX_EXTRACTED_SIZE {
            return Err(invalid("release archive expands beyond safety limits"));
        }

        if matches!(
            path.to_str().map(|value| value.trim_start_matches("./")),
            Some("codex-ticket-probe" | "VERSION" | "REVISION")
        ) {
            let target = temp_dir.join(path.file_name().expect("companion file"));
            if !destinations.insert(target.clone()) {
                return Err(invalid("release archive contains duplicate files"));
            }
            entry
                .unpack(&target)
                .map_err(|error| internal(format!("failed to extract companion: {error}")))?;
            companions.push(target);
            continue;
        }
        if path.file_name().is_some_and(|name| name == APP_BINARY_NAME) {
            if found_binary {
                return Err(invalid("release archive contains duplicate binaries"));
            }
            entry
                .unpack(&binary_path)
                .map_err(|error| internal(format!("failed to extract binary: {error}")))?;
            found_binary = true;
            continue;
        }
        if let Some(relative) = modeltrace_relative_path(&path) {
            if relative.as_os_str().is_empty() {
                continue;
            }
            let target = modeltrace_dir.join(relative);
            if !destinations.insert(target.clone()) {
                return Err(invalid("release archive contains duplicate ModelTrace files"));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    internal(format!("failed to create ModelTrace directory: {error}"))
                })?;
            }
            entry
                .unpack(&target)
                .map_err(|error| internal(format!("failed to extract ModelTrace file: {error}")))?;
            found_modeltrace = true;
            continue;
        }
        if let Some(relative) = web_dist_relative_path(&path) {
            if relative.as_os_str().is_empty() {
                continue;
            }
            let target = web_dist_dir.join(relative);
            if !destinations.insert(target.clone()) {
                return Err(invalid("release archive contains duplicate web assets"));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    internal(format!("failed to create web asset dir: {error}"))
                })?;
            }
            entry
                .unpack(&target)
                .map_err(|error| internal(format!("failed to extract web asset: {error}")))?;
            found_web = true;
        }
    }
    if !found_binary {
        return Err(invalid("release archive does not contain codex-proxy-rs"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&binary_path, fs::Permissions::from_mode(0o755))
            .map_err(|error| internal(format!("failed to chmod binary: {error}")))?;
    }
    Ok(ExtractedRelease {
        binary_path,
        web_dist_dir: found_web.then_some(web_dist_dir),
        modeltrace_dir: found_modeltrace.then_some(modeltrace_dir),
        companions,
    })
}

fn unsafe_archive_path(path: &Path) -> bool {
    path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
}

fn modeltrace_relative_path(path: &Path) -> Option<PathBuf> {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    (components.first().is_some_and(|value| value == "modeltrace"))
        .then(|| components[1..].iter().collect())
}

fn web_dist_relative_path(path: &Path) -> Option<PathBuf> {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for index in 0..components.len() {
        if components[index] == "web"
            && components
                .get(index + 1)
                .is_some_and(|value| value == "dist")
        {
            return Some(components[index + 2..].iter().collect());
        }
        if components[index] == "dist" {
            return Some(components[index + 1..].iter().collect());
        }
    }
    None
}
