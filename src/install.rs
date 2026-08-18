//! Install the running binary as the `mdb` CLI into a directory on `PATH`.

use std::path::PathBuf;

/// Outcome of a CLI-install attempt, surfaced in the settings page.
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub ok: bool,
    pub message: String,
}

/// Copy the current executable to `mdb` inside the first writable, PATH-listed
/// directory (falling back to `~/.local/bin`).
pub fn install_cli() -> InstallResult {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            return InstallResult {
                ok: false,
                message: format!("无法定位当前程序: {e}"),
            }
        }
    };

    let Some(dir) = install_dir() else {
        return InstallResult {
            ok: false,
            message: "无法确定安装目录".into(),
        };
    };

    if let Err(e) = std::fs::create_dir_all(&dir) {
        return InstallResult {
            ok: false,
            message: format!("创建目录失败: {e}"),
        };
    }

    let dest = dir.join("mdb");
    if let Err(e) = std::fs::copy(&exe, &dest) {
        return InstallResult {
            ok: false,
            message: format!("复制失败: {e}"),
        };
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
    }

    InstallResult {
        ok: true,
        message: format!("已安装: {}", dest.display()),
    }
}

/// Prefer a directory already listed in `PATH`, else `~/.local/bin`.
fn install_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let path_entries: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();

    let candidates = [
        home.join(".local/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join("bin"),
    ];

    for dir in candidates {
        if path_entries.iter().any(|p| p == &dir) {
            return Some(dir);
        }
    }

    // Not on PATH yet (common on stock macOS): install anyway.
    Some(home.join(".local/bin"))
}
