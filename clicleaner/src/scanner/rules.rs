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
#[allow(clippy::too_many_arguments)]
pub fn classify_directory(
    dir_path: &Path,
    dir_name: &str,
    scan_node: bool,
    scan_nuget: bool,
    scan_rust: bool,
    scan_github: bool,
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
        "target" if scan_rust => {
            if has_rust_project_context(dir_path) {
                Some(ArtifactKind::RustTarget)
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
        ".github-packages" | "github-packages" | "gh-packages" | ".act" if scan_github => {
            Some(ArtifactKind::GitHubPackages)
        }
        "packages" | "cache" | ".cache" | "artifacts"
            if scan_github && is_inside_github_dir(dir_path) =>
        {
            Some(ArtifactKind::GitHubPackages)
        }
        "packages" => {
            let is_github = scan_github && is_github_packages_source_directory(dir_path);
            let is_nuget = scan_nuget && is_nuget_packages_directory(dir_path);
            if is_github && !is_nuget {
                Some(ArtifactKind::GitHubPackages)
            } else if is_nuget {
                Some(ArtifactKind::NuGetPackages)
            } else if is_github {
                Some(ArtifactKind::GitHubPackages)
            } else {
                None
            }
        }
        "nupkgs" | "nupkg" | ".nuget-packages" | "nuget-packages" if scan_nuget => {
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

/// Checks if a `target` directory belongs to a Rust / Cargo project or workspace.
pub fn has_rust_project_context(target_path: &Path) -> bool {
    let Some(parent) = target_path.parent() else {
        return false;
    };

    if parent.join("Cargo.toml").is_file() || parent.join("Cargo.lock").is_file() {
        return true;
    }

    // Also detect Cargo target directories placed in a nested build folder when Cargo markers exist nearby
    let has_cargo_target_marker = target_path.join("CACHEDIR.TAG").is_file()
        || target_path.join(".rustc_info.json").is_file();

    if has_cargo_target_marker {
        let mut ancestor = parent.parent();
        for _ in 0..3 {
            let Some(dir) = ancestor else {
                break;
            };
            if dir.join("Cargo.toml").is_file() {
                return true;
            }
            ancestor = dir.parent();
        }
    }

    false
}

/// Returns true if `dir_path` is an immediate child of a `.github` directory.
pub fn is_inside_github_dir(dir_path: &Path) -> bool {
    dir_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some(".github")
}

/// Checks whether a `packages` directory is associated with a GitHub Packages registry configuration.
pub fn is_github_packages_source_directory(dir_path: &Path) -> bool {
    if has_workspace_project_children(dir_path) {
        return false;
    }

    let Some(parent) = dir_path.parent() else {
        return false;
    };

    const CONFIG_FILES: &[&str] = &[".npmrc", "nuget.config", "NuGet.Config"];
    CONFIG_FILES.iter().any(|file_name| {
        let config_path = parent.join(file_name);
        config_path.is_file()
            && fs::read_to_string(&config_path)
                .map(|contents| contents.to_ascii_lowercase().contains("pkg.github.com"))
                .unwrap_or(false)
    })
}

/// Checks if a `bin` or `obj` directory belongs to a .NET / NuGet project by inspecting its parent directory
/// for `.csproj`, `.fsproj`, `.vbproj`, `project.json`, `packages.config`, `Directory.Build.props`, or `Directory.Packages.props`.
pub fn has_dotnet_project_context(dir_path: &Path) -> bool {
    let Some(parent) = dir_path.parent() else {
        return false;
    };

    if parent.join("project.json").is_file()
        || parent.join("packages.config").is_file()
        || parent.join("Directory.Build.props").is_file()
        || parent.join("Directory.Packages.props").is_file()
    {
        return true;
    }

    has_dotnet_project_file(parent, false)
}

fn has_dotnet_project_file(dir: &Path, include_solutions: bool) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("csproj")
                    || ext.eq_ignore_ascii_case("fsproj")
                    || ext.eq_ignore_ascii_case("vbproj")
                    || (include_solutions
                        && (ext.eq_ignore_ascii_case("sln") || ext.eq_ignore_ascii_case("slnx")))
                {
                    return true;
                }
            }
        }
    }

    false
}

fn is_workspace_project_dir(dir: &Path) -> bool {
    dir.join("package.json").is_file()
        || dir.join("Cargo.toml").is_file()
        || has_dotnet_project_file(dir, true)
}

fn has_workspace_project_children(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let child_path = entry.path();
        if child_path.is_dir() && is_workspace_project_dir(&child_path) {
            return true;
        }
    }

    false
}

/// Verifies whether a `packages` or NuGet feed directory is a NuGet external package folder rather than a source-code workspace.
pub fn is_nuget_packages_directory(packages_path: &Path) -> bool {
    // 1. Direct marker inside `packages/` (e.g., `repositories.config` or `.nuget/packages`)
    if packages_path.join("repositories.config").is_file() {
        return true;
    }

    if packages_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some(".nuget")
    {
        return true;
    }

    let mut has_nupkg_child = false;
    let mut has_workspace_child = false;

    if let Ok(entries) = fs::read_dir(packages_path) {
        for entry in entries.flatten() {
            let child_path = entry.path();
            if child_path.is_file() {
                if is_nupkg_artifact_file(&child_path) {
                    has_nupkg_child = true;
                }
            } else if child_path.is_dir() {
                // If child folder contains project markers, this is likely a monorepo `packages/` workspace
                if is_workspace_project_dir(&child_path) {
                    has_workspace_child = true;
                }
                if directory_contains_nupkg(&child_path) {
                    has_nupkg_child = true;
                }
            }
        }
    }

    if has_workspace_child {
        return false;
    }
    if has_nupkg_child {
        return true;
    }

    // 2. Check parent directory for `.sln`, `.slnx`, `.csproj`, `nuget.config`, or `packages.config`
    let Some(parent) = packages_path.parent() else {
        return false;
    };

    if parent.join("packages.config").is_file()
        || parent.join("nuget.config").is_file()
        || parent.join("NuGet.Config").is_file()
    {
        return true;
    }

    has_dotnet_project_file(parent, true)
}

fn is_nupkg_artifact_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".nupkg")
        || lower.ends_with(".snupkg")
        || lower.ends_with(".nupkg.sha512")
        || lower.ends_with(".nupkg.metadata")
}

fn directory_contains_nupkg(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if is_nupkg_artifact_file(&path) {
                return true;
            }
        } else if path.is_dir() {
            // Check one level deeper for hierarchical NuGet caches (`<package-id>/<version>/*.nupkg`)
            if let Ok(nested_entries) = fs::read_dir(&path) {
                for nested in nested_entries.flatten() {
                    let nested_path = nested.path();
                    if nested_path.is_file() && is_nupkg_artifact_file(&nested_path) {
                        return true;
                    }
                }
            }
        }
    }

    false
}
