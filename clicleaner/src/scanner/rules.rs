use std::fs;
use std::path::Path;

use crate::scanner::ArtifactKind;

/// System directories that must never be scanned or cleaned.
const PROTECTED_UNIX_PATHS: &[&str] = &[
    "/", "/System", "/usr", "/bin", "/sbin", "/Library", "/etc", "/dev", "/proc", "/sys",
];

const PROTECTED_WINDOWS_PATHS: &[&str] = &[
    r"C:\",
    r"C:\Windows",
    r"C:\Program Files",
    r"C:\Program Files (x86)",
];

/// VCS directories that should always be skipped during directory walking.
const SKIPPED_VCS_DIRS: &[&str] = &[".git", ".hg", ".svn"];

/// Checks whether a canonicalized path is a protected operating system path.
pub fn is_protected_system_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    for &protected in PROTECTED_UNIX_PATHS {
        if path == Path::new(protected)
            || (protected != "/" && path_str.starts_with(&format!("{protected}/")))
        {
            return true;
        }
    }

    for &protected in PROTECTED_WINDOWS_PATHS {
        if path_str.eq_ignore_ascii_case(protected) {
            return true;
        }
    }

    false
}

/// Returns true if the directory name is a hidden VCS folder that should not be traversed.
pub fn is_vcs_directory(dir_name: &str) -> bool {
    SKIPPED_VCS_DIRS.contains(&dir_name)
}

/// Evaluates whether a directory at `dir_path` with name `dir_name` matches a cleanable target.
pub fn classify_directory(
    dir_path: &Path,
    dir_name: &str,
    scan_node: bool,
    scan_nuget: bool,
    include_obj: bool,
    strict_node: bool,
) -> Option<ArtifactKind> {
    match dir_name {
        "node_modules" if scan_node => {
            if !strict_node || has_node_project_context(dir_path) {
                Some(ArtifactKind::NodeModules)
            } else {
                None
            }
        }
        "bin" if scan_nuget => {
            if has_dotnet_project_context(dir_path) {
                Some(ArtifactKind::DotNetBin)
            } else {
                None
            }
        }
        "obj" if scan_nuget && include_obj => {
            if has_dotnet_project_context(dir_path) {
                Some(ArtifactKind::DotNetObj)
            } else {
                None
            }
        }
        "packages" if scan_nuget => {
            if is_nuget_packages_directory(dir_path) {
                Some(ArtifactKind::NuGetPackages)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Checks if `node_modules` has a sibling Node.js project marker (`package.json` or lockfiles).
pub fn has_node_project_context(node_modules_path: &Path) -> bool {
    let Some(parent) = node_modules_path.parent() else {
        return false;
    };

    const NODE_MARKERS: &[&str] = &[
        "package.json",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "bun.lockb",
        "bun.lock",
    ];

    NODE_MARKERS
        .iter()
        .any(|marker| parent.join(marker).is_file())
}

/// Checks if a `bin` or `obj` directory belongs to a .NET / NuGet project by inspecting its parent directory
/// for `.csproj`, `.fsproj`, `.vbproj`, `project.json`, or `packages.config`.
pub fn has_dotnet_project_context(dir_path: &Path) -> bool {
    let Some(parent) = dir_path.parent() else {
        return false;
    };

    if parent.join("project.json").is_file() || parent.join("packages.config").is_file() {
        return true;
    }

    let Ok(entries) = fs::read_dir(parent) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("csproj")
                    || ext.eq_ignore_ascii_case("fsproj")
                    || ext.eq_ignore_ascii_case("vbproj")
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Verifies whether a `packages` directory is a NuGet external package folder rather than a source-code workspace.
pub fn is_nuget_packages_directory(packages_path: &Path) -> bool {
    // 1. Direct marker inside `packages/` (e.g., `repositories.config` or `.nupkg` files in immediate children)
    if packages_path.join("repositories.config").is_file() {
        return true;
    }

    let mut has_nupkg_child = false;
    let mut has_js_workspace_child = false;

    if let Ok(entries) = fs::read_dir(packages_path) {
        for entry in entries.flatten() {
            let child_path = entry.path();
            if child_path.is_file() {
                if let Some(ext) = child_path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("nupkg") {
                        return true;
                    }
                }
            } else if child_path.is_dir() {
                // If child folder has a package.json, this is likely a JS/TS monorepo `packages/` workspace
                if child_path.join("package.json").is_file() {
                    has_js_workspace_child = true;
                }
                if directory_contains_nupkg(&child_path) {
                    has_nupkg_child = true;
                }
            }
        }
    }

    if has_nupkg_child {
        return true;
    }
    if has_js_workspace_child {
        return false;
    }

    // 2. Check parent directory for `.sln`, `.slnx`, `nuget.config`, or `packages.config`
    let Some(parent) = packages_path.parent() else {
        return false;
    };

    if parent.join("packages.config").is_file()
        || parent.join("nuget.config").is_file()
        || parent.join("NuGet.Config").is_file()
    {
        return true;
    }

    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("sln")
                        || ext.eq_ignore_ascii_case("slnx")
                        || ext.eq_ignore_ascii_case("csproj")
                        || ext.eq_ignore_ascii_case("fsproj")
                        || ext.eq_ignore_ascii_case("vbproj")
                    {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn directory_contains_nupkg(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_ascii_lowercase();
                if lower.ends_with(".nupkg") || lower.ends_with(".nupkg.sha512") {
                    return true;
                }
            }
        }
    }

    false
}
