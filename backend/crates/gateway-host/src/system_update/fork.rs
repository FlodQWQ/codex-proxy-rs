//! 定制包的完整校验和整组交换；不执行发布包中的脚本，不改配置或数据库。

use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use super::archive::ExtractedRelease;
use super::state::UpdateTempDir;
use super::swap::{copy_dir_all, swap_dir, swap_file};
use super::{OperationError, SystemUpdateConfig, conflict, internal, invalid};

const FILES: [&str; 4] = [
    "codex-proxy-rs",
    "codex-ticket-probe",
    "VERSION",
    "REVISION",
];

fn targets(config: &SystemUpdateConfig) -> Result<Vec<PathBuf>, OperationError> {
    let executable = config.executable_path()?;
    let root = executable
        .parent()
        .ok_or_else(|| invalid("missing install directory"))?;
    if executable.file_name().is_none_or(|name| name != FILES[0])
        || config.web_dist_dir()? != root.join("web/dist")
    {
        return Err(conflict("定制包需要同目录的主程序、探针及 web/dist 布局"));
    }
    let mut paths = vec![executable.clone()];
    paths.extend(FILES[1..].iter().map(|name| root.join(name)));
    paths.push(config.web_dist_dir()?.to_owned());
    for (index, path) in paths.iter().enumerate() {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            conflict(format!(
                "定制包当前文件不可访问 {}: {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink()
            || (index == FILES.len() && !metadata.is_dir())
            || (index < FILES.len() && !metadata.is_file())
        {
            return Err(conflict("定制包当前文件类型不安全"));
        }
    }
    Ok(paths)
}

fn bundle_paths(root: &Path) -> Vec<PathBuf> {
    FILES
        .iter()
        .chain(std::iter::once(&"web-dist"))
        .map(|name| root.join(name))
        .collect()
}

fn elf(path: &Path) -> Result<(), OperationError> {
    let mut header = [0_u8; 20];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|error| invalid(format!("invalid ELF: {error}")))?;
    if &header[..6] != b"\x7fELF\x02\x01" || header[18..20] != [0x3e, 0] {
        return Err(invalid("定制包必须包含 Linux x86_64 主程序与探针"));
    }
    Ok(())
}

fn exchange(current: &[PathBuf], replacement: &[PathBuf]) -> Result<(), OperationError> {
    for index in 0..current.len() {
        if let Err(error) = swap(&current[index], &replacement[index], index == FILES.len()) {
            let mut failures = Vec::new();
            for previous in (0..index).rev() {
                if let Err(error) = swap(
                    &current[previous],
                    &replacement[previous],
                    previous == FILES.len(),
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

pub(super) fn install(
    config: &SystemUpdateConfig,
    extracted: ExtractedRelease,
    version: &str,
) -> Result<(), OperationError> {
    let current = targets(config)?;
    let root = current[0].parent().expect("validated root");
    let source_root = extracted.binary_path.parent().expect("extracted root");
    if extracted.companions.len() != 3 {
        return Err(invalid("定制包缺少探针、VERSION 或 REVISION"));
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
    elf(&source_root.join("codex-ticket-probe"))?;
    let web = extracted
        .web_dist_dir
        .ok_or_else(|| invalid("定制包缺少前端"))?;
    if !web.join("index.html").is_file() {
        return Err(invalid("定制包缺少 index.html"));
    }

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
    let backup = root.join(".fork-update-backup");
    let previous = stage.path().with_extension("previous-backup");
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
    }
    result
}

pub(super) fn rollback(config: &SystemUpdateConfig) -> Result<(), OperationError> {
    let current = targets(config)?;
    let backup = current[0]
        .parent()
        .expect("validated root")
        .join(".fork-update-backup");
    let replacements = bundle_paths(&backup);
    for (index, path) in replacements.iter().enumerate() {
        let metadata = fs::symlink_metadata(path).map_err(|_| conflict("没有完整的定制包备份"))?;
        if metadata.file_type().is_symlink()
            || (index < FILES.len() && !metadata.is_file())
            || (index == FILES.len() && !metadata.is_dir())
        {
            return Err(conflict("定制包备份不完整"));
        }
    }
    exchange(&current, &replacements)
}
