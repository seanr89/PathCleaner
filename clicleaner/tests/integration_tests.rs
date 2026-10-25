use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn create_file_with_size(path: &Path, size: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let data = vec![b'x'; size];
    fs::write(path, data).unwrap();
}

#[test]
fn test_scans_and_cleans_node_modules_and_nuget_folders_safely() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Node.js project with nested node_modules
    let web_app = root.join("webapp");
    create_file_with_size(&web_app.join("package.json"), 120);
    create_file_with_size(
        &web_app.join("node_modules/react/index.js"),
        4_096,
    );
    create_file_with_size(
        &web_app.join("node_modules/react/node_modules/loose-envify/index.js"),
        2_048,
    );
    // Generic script bin folder inside webapp (MUST NOT be deleted!)
    create_file_with_size(&web_app.join("bin/deploy.sh"), 512);

    // 2. .NET / NuGet project with .csproj, bin, obj, and solution packages
    let dotnet_sln = root.join("BillingService");
    create_file_with_size(&dotnet_sln.join("BillingService.sln"), 256);
    create_file_with_size(
        &dotnet_sln.join("packages/Newtonsoft.Json.13.0.3/Newtonsoft.Json.13.0.3.nupkg"),
        8_192,
    );

    let api_proj = dotnet_sln.join("src/Billing.Api");
    create_file_with_size(&api_proj.join("Billing.Api.csproj"), 512);
    create_file_with_size(&api_proj.join("bin/Debug/net8.0/Billing.Api.dll"), 16_384);
    create_file_with_size(&api_proj.join("obj/project.assets.json"), 4_096);

    // 3. JS monorepo `packages/` workspace (MUST NOT be deleted!)
    let monorepo = root.join("monorepo");
    create_file_with_size(&monorepo.join("packages/ui-kit/package.json"), 200);
    create_file_with_size(&monorepo.join("packages/ui-kit/src/button.tsx"), 1_024);

    // First run with --dry-run
    let mut dry_cmd = Command::cargo_bin("clicleaner").unwrap();
    dry_cmd
        .arg(root)
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("node_modules"))
        .stdout(predicate::str::contains("BillingService/packages"))
        .stdout(predicate::str::contains("Billing.Api/bin"))
        .stdout(predicate::str::contains("Billing.Api/obj"));

    // Verify dry-run did not delete anything
    assert!(web_app.join("node_modules").exists());
    assert!(dotnet_sln.join("packages").exists());
    assert!(api_proj.join("bin").exists());
    assert!(api_proj.join("obj").exists());

    // Now run actual cleanup with --yes
    let mut clean_cmd = Command::cargo_bin("clicleaner").unwrap();
    clean_cmd
        .arg(root)
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted 4 directories"));

    // Verify targets were deleted
    assert!(!web_app.join("node_modules").exists());
    assert!(!dotnet_sln.join("packages").exists());
    assert!(!api_proj.join("bin").exists());
    assert!(!api_proj.join("obj").exists());

    // Verify safe directories and source files were untouched!
    assert!(web_app.join("package.json").exists());
    assert!(web_app.join("bin/deploy.sh").exists());
    assert!(api_proj.join("Billing.Api.csproj").exists());
    assert!(monorepo.join("packages/ui-kit/src/button.tsx").exists());
}

#[test]
fn test_target_ecosystem_filter_and_json_output() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Node project
    create_file_with_size(&root.join("app/package.json"), 100);
    create_file_with_size(&root.join("app/node_modules/pkg/index.js"), 1024);

    // .NET project
    create_file_with_size(&root.join("svc/Svc.csproj"), 100);
    create_file_with_size(&root.join("svc/bin/Release/Svc.dll"), 2048);

    // Clean ONLY node_modules via `--target node --yes --json`
    let output = Command::cargo_bin("clicleaner")
        .unwrap()
        .arg(root)
        .arg("--target")
        .arg("node")
        .arg("--yes")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["discovered_targets"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["discovered_targets"][0]["kind"].as_str().unwrap(),
        "node_modules"
    );

    // Verify node_modules was deleted while .NET bin remains intact
    assert!(!root.join("app/node_modules").exists());
    assert!(root.join("svc/bin").exists());
}
