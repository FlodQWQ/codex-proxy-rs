//! 定制包的完整校验和整组交换；不执行发布包中的脚本，不改配置或数据库。

use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::archive::ExtractedRelease;
use super::state::UpdateTempDir;
use super::swap::{copy_dir_all, swap_dir, swap_file};
use super::{OperationError, SystemUpdateConfig, conflict, internal, invalid};

const FILES: [&str; 3] = ["codex-proxy-rs", "VERSION", "REVISION"];

fn targets(
    config: &SystemUpdateConfig,
    require_official_plugins: bool,
) -> Result<Vec<PathBuf>, OperationError> {
    let executable = config.executable_path()?;
    let root = executable
        .parent()
        .ok_or_else(|| invalid("missing install directory"))?;
    if executable.file_name().is_none_or(|name| name != FILES[0])
        || config.web_dist_dir()? != root.join("web/dist")
    {
        return Err(conflict("定制包需要同目录的主程序及 web/dist 布局"));
    }
    let mut paths = vec![executable.clone()];
    paths.extend(FILES[1..].iter().map(|name| root.join(name)));
    paths.push(config.web_dist_dir()?.to_owned());
    paths.push(config.official_plugins_dir()?);
    for (index, path) in paths.iter().enumerate() {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error)
                if !require_official_plugins
                    && index + 1 == paths.len()
                    && error.kind() == io::ErrorKind::NotFound =>
            {
                continue;
            }
            Err(error) => {
                return Err(conflict(format!(
                    "定制包当前文件不可访问 {}: {error}",
                    path.display()
                )));
            }
        };
        if metadata.file_type().is_symlink()
            || (index >= FILES.len() && !metadata.is_dir())
            || (index < FILES.len() && !metadata.is_file())
        {
            return Err(conflict("定制包当前文件类型不安全"));
        }
    }
    Ok(paths)
}

fn ensure_official_plugins_dir(config: &SystemUpdateConfig) -> Result<bool, OperationError> {
    let official = config.official_plugins_dir()?;
    let parent = official
        .parent()
        .ok_or_else(|| invalid("missing official plugin parent directory"))?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(conflict("定制包官方插件父目录类型不安全"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(parent).map_err(|error| {
                internal(format!(
                    "failed to create official plugin parent directory: {error}"
                ))
            })?;
        }
        Err(error) => {
            return Err(conflict(format!("定制包官方插件父目录不可访问: {error}")));
        }
    }
    match fs::symlink_metadata(&official) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(conflict("定制包官方插件目录类型不安全"))
        }
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&official).map(|()| true).map_err(|error| {
                internal(format!(
                    "failed to create official plugin directory: {error}"
                ))
            })
        }
        Err(error) => Err(conflict(format!("定制包官方插件目录不可访问: {error}"))),
    }
}

fn bundle_paths(root: &Path) -> Vec<PathBuf> {
    FILES
        .iter()
        .chain(["web-dist", "plugins/official"].iter())
        .map(|name| root.join(name))
        .collect()
}

fn elf(path: &Path) -> Result<(), OperationError> {
    let mut header = [0_u8; 20];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|error| invalid(format!("invalid ELF: {error}")))?;
    if &header[..6] != b"\x7fELF\x02\x01" || header[18..20] != [0x3e, 0] {
        return Err(invalid("定制包必须包含 Linux x86_64 主程序"));
    }
    Ok(())
}

fn exchange(current: &[PathBuf], replacement: &[PathBuf]) -> Result<(), OperationError> {
    for index in 0..current.len() {
        if let Err(error) = swap(&current[index], &replacement[index], index >= FILES.len()) {
            let mut failures = Vec::new();
            for previous in (0..index).rev() {
                if let Err(error) = swap(
                    &current[previous],
                    &replacement[previous],
                    previous >= FILES.len(),
                ) {
                    failures.push(error.to_string());
                }
            }
            return Err(internal(format!(
                "整包替换失败: {error}; 恢复错误: {failures:?}"
            )));
        }
    }
    Ok(())
}

fn swap(current: &Path, replacement: &Path, directory: bool) -> io::Result<()> {
    if directory {
        swap_dir(current, replacement)
    } else {
        swap_file(current, replacement)
    }
}

const MODELTRACE_REVISION: &str = "55a2e4a55170423b484d701e9a82ab62b268c811";

fn valid_modeltrace_tree(path: &Path) -> bool {
    [
        ".git/HEAD",
        "challenge_suite.py",
        "fingerprint.py",
        "data/unified_bank.json",
    ]
    .iter()
    .all(|file| path.join(file).is_file())
}

fn modeltrace_revision(path: &Path) -> Option<String> {
    if let Ok(revision) = fs::read_to_string(path.join(".git/CPR_REVISION")) {
        return Some(revision.trim().to_owned());
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn install_modeltrace(runtime_data_dir: &Path, source: &Path) -> Result<(), OperationError> {
    if !valid_modeltrace_tree(source)
        || modeltrace_revision(source).as_deref() != Some(MODELTRACE_REVISION)
    {
        return Err(invalid("Release 缺少固定版本的 ModelTrace 指纹库"));
    }
    fs::create_dir_all(runtime_data_dir)
        .map_err(|error| internal(format!("failed to prepare runtime data: {error}")))?;
    let runtime_metadata = fs::symlink_metadata(runtime_data_dir)
        .map_err(|error| internal(format!("failed to inspect runtime data: {error}")))?;
    if runtime_metadata.file_type().is_symlink() || !runtime_metadata.is_dir() {
        return Err(conflict(
            "runtime data directory is not a regular directory",
        ));
    }

    let target = runtime_data_dir.join("modeltrace");
    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || !valid_modeltrace_tree(&target)
                || modeltrace_revision(&target).as_deref() != Some(MODELTRACE_REVISION)
            {
                return Err(conflict(
                    "ModelTrace directory already exists with unexpected content; it was not overwritten",
                ));
            }
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(internal(format!("failed to inspect ModelTrace: {error}"))),
    }

    let stage = UpdateTempDir::create(runtime_data_dir)?;
    let staged_modeltrace = stage.path().join("modeltrace");
    copy_dir_all(source, &staged_modeltrace)
        .map_err(|error| internal(format!("failed to stage ModelTrace checkout: {error}")))?;
    fs::rename(&staged_modeltrace, &target)
        .map_err(|error| internal(format!("failed to install ModelTrace checkout: {error}")))
}

pub(super) fn install(
    config: &SystemUpdateConfig,
    extracted: ExtractedRelease,
    version: &str,
) -> Result<(), OperationError> {
    let current = targets(config, false)?;
    let root = current[0].parent().expect("validated root");
    let source_root = extracted.binary_path.parent().expect("extracted root");
    if extracted.companions.len() != 2 {
        return Err(invalid("定制包缺少 VERSION 或 REVISION"));
    }
    let package_version = fs::read_to_string(source_root.join("VERSION"))
        .map_err(|error| invalid(error.to_string()))?;
    let revision = fs::read_to_string(source_root.join("REVISION"))
        .map_err(|error| invalid(error.to_string()))?;
    if package_version.trim() != version
        || revision.trim().len() != 40
        || !revision.trim().bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid("定制包版本或提交号不匹配"));
    }
    elf(&extracted.binary_path)?;
    let web = extracted
        .web_dist_dir
        .ok_or_else(|| invalid("定制包缺少前端"))?;
    let official_plugins = extracted.official_plugins_dir;
    let manifest = official_plugins.join("plugin-release-manifest.json");
    let official_metadata = fs::symlink_metadata(&official_plugins)
        .map_err(|error| invalid(format!("定制包缺少官方插件目录: {error}")))?;
    if official_metadata.file_type().is_symlink() || !official_metadata.is_dir() {
        return Err(invalid("定制包官方插件目录类型不安全"));
    }
    let manifest_metadata = fs::symlink_metadata(&manifest)
        .map_err(|error| invalid(format!("定制包缺少官方插件清单: {error}")))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err(invalid("定制包官方插件清单类型不安全"));
    }
    let modeltrace = extracted
        .modeltrace_dir
        .ok_or_else(|| invalid("定制包缺少 ModelTrace 指纹库"))?;
    if !web.join("index.html").is_file() {
        return Err(invalid("定制包缺少 index.html"));
    }
    install_modeltrace(
        config
            .runtime_data_dir
            .as_deref()
            .ok_or_else(|| conflict("runtime data directory is not configured"))?,
        &modeltrace,
    )?;

    // 在部署所在文件系统准备完整新包，再交换，避免跨文件系统复制发生在替换期间。
    let stage = UpdateTempDir::create(root)?;
    let staged = bundle_paths(stage.path());
    for (index, name) in FILES.iter().enumerate() {
        fs::copy(source_root.join(name), &staged[index])
            .map_err(|error| internal(error.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(
                &staged[index],
                fs::Permissions::from_mode(if index < 2 { 0o755 } else { 0o644 }),
            )
            .map_err(|error| internal(error.to_string()))?;
        }
    }
    copy_dir_all(&web, &staged[FILES.len()]).map_err(|error| internal(error.to_string()))?;
    copy_dir_all(&official_plugins, &staged[FILES.len() + 1])
        .map_err(|error| internal(error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&staged[FILES.len() + 1], fs::Permissions::from_mode(0o555))
            .map_err(|error| internal(error.to_string()))?;
    }
    let backup = root.join(".fork-update-backup");
    let previous = stage.path().with_extension("previous-backup");
    let created_official_plugins = ensure_official_plugins_dir(config)?;
    let had_backup = backup.exists();
    if had_backup {
        fs::rename(&backup, &previous)
            .map_err(|error| internal(format!("备份不可移动: {error}")))?;
    }
    let result = exchange(&current, &staged).and_then(|()| {
        if let Err(error) = fs::rename(stage.path(), &backup) {
            exchange(&current, &staged)?;
            return Err(internal(format!("备份提交失败: {error}")));
        }
        Ok(())
    });
    if result.is_err() {
        // 保留失败现场，恢复遇到磁盘故障时也不能删除仍包含旧文件的目录。
        std::mem::forget(stage);
        if had_backup {
            fs::rename(&previous, &backup)
                .map_err(|error| internal(format!("旧备份恢复失败: {error}")))?;
        }
        if created_official_plugins {
            let official = config.official_plugins_dir()?;
            if fs::read_dir(&official)
                .ok()
                .is_some_and(|mut entries| entries.next().is_none())
            {
                let _ = fs::remove_dir(&official);
            }
        }
    }
    result
}

pub(super) fn rollback(config: &SystemUpdateConfig) -> Result<(), OperationError> {
    let current = targets(config, true)?;
    let backup = current[0]
        .parent()
        .expect("validated root")
        .join(".fork-update-backup");
    let replacements = bundle_paths(&backup);
    for (index, path) in replacements.iter().enumerate() {
        let metadata = fs::symlink_metadata(path).map_err(|_| conflict("没有完整的定制包备份"))?;
        if metadata.file_type().is_symlink()
            || (index < FILES.len() && !metadata.is_file())
            || (index >= FILES.len() && !metadata.is_dir())
        {
            return Err(conflict("定制包备份不完整"));
        }
    }
    exchange(&current, &replacements)
}

pub(super) fn rollback_official_plugins_dir(
    config: &SystemUpdateConfig,
) -> Result<PathBuf, OperationError> {
    let executable = config.executable_path()?;
    let root = executable
        .parent()
        .ok_or_else(|| invalid("missing install directory"))?;
    Ok(root.join(".fork-update-backup/plugins/official"))
}
