use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::vscode;

pub const REPOSITORY: &str = "https://github.com/hwtnb/translation-lsp";

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

pub fn output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root())
        .output()
        .with_context(|| format!("Starting {program}"))?;
    ensure!(
        output.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

pub fn git(args: &[&str]) -> Result<String> {
    Ok(output("git", args)?.trim().to_owned())
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    Aarch64,
    X86_64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub os: Os,
    pub arch: Arch,
    pub target: String,
}

impl Target {
    /// The GitHub Actions runner that builds and tests this target.
    fn runner(&self) -> &'static str {
        match (self.os, self.arch) {
            (Os::Mac, Arch::Aarch64) => "macos-15",
            (Os::Mac, Arch::X86_64) => "macos-15-intel",
            (Os::Linux, Arch::Aarch64) => "ubuntu-22.04-arm",
            (Os::Linux, Arch::X86_64) => "ubuntu-22.04",
            (Os::Windows, Arch::Aarch64) => "windows-11-arm",
            (Os::Windows, Arch::X86_64) => "windows-2025",
        }
    }
    /// The target that `vsce` names this one by, for the VS Code extension package.
    pub fn vscode_target(&self) -> &'static str {
        match (self.os, self.arch) {
            (Os::Mac, Arch::Aarch64) => "darwin-arm64",
            (Os::Mac, Arch::X86_64) => "darwin-x64",
            (Os::Linux, Arch::Aarch64) => "linux-arm64",
            (Os::Linux, Arch::X86_64) => "linux-x64",
            (Os::Windows, Arch::Aarch64) => "win32-arm64",
            (Os::Windows, Arch::X86_64) => "win32-x64",
        }
    }
    /// Whether this is the target that runs `xtask` itself.
    pub fn is_host(&self) -> bool {
        let os = if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else if cfg!(windows) {
            Os::Windows
        } else {
            return false;
        };
        let arch = if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else if cfg!(target_arch = "x86_64") {
            Arch::X86_64
        } else {
            return false;
        };
        (self.os, self.arch) == (os, arch)
    }
    pub fn executable(&self) -> &'static str {
        if self.os == Os::Windows {
            "translation-lsp.exe"
        } else {
            "translation-lsp"
        }
    }
    pub fn archive_name(&self, version: &str) -> String {
        let suffix = if self.os == Os::Windows {
            "zip"
        } else {
            "tar.gz"
        };
        format!("translation-lsp-v{version}-{}.{suffix}", self.target)
    }
    pub fn vsix_name(&self, version: &str) -> String {
        format!(
            "vscode-translation-lsp-v{version}-{}.vsix",
            self.vscode_target()
        )
    }
}

pub fn targets() -> Result<Vec<Target>> {
    #[derive(Deserialize)]
    struct Matrix {
        include: Vec<Target>,
    }
    let matrix: Matrix = serde_json::from_str(&fs::read_to_string(
        root().join("zed-extension/release-targets.json"),
    )?)?;
    let mut pairs = BTreeSet::new();
    for item in &matrix.include {
        let suffix = match item.os {
            Os::Mac => "apple-darwin",
            Os::Linux => "unknown-linux-gnu",
            Os::Windows => "pc-windows-msvc",
        };
        let prefix = match item.arch {
            Arch::Aarch64 => "aarch64",
            Arch::X86_64 => "x86_64",
        };
        ensure!(
            item.target == format!("{prefix}-{suffix}"),
            "Target does not match OS and architecture"
        );
        ensure!(pairs.insert((item.os, item.arch)), "Duplicate target");
    }
    ensure!(
        pairs.len() == 6,
        "Expected both architectures on all three OSes"
    );
    Ok(matrix.include)
}

pub fn matrix() -> Result<serde_json::Value> {
    let include: Vec<_> = targets()?
        .iter()
        .map(|target| {
            serde_json::json!({
                "os": target.os,
                "arch": target.arch,
                "target": target.target,
                "runner": target.runner(),
            })
        })
        .collect();
    Ok(serde_json::json!({ "include": include }))
}

pub fn target(name: &str) -> Result<Target> {
    targets()?
        .into_iter()
        .find(|t| t.target == name)
        .with_context(|| format!("Unknown target {name}"))
}

#[derive(Deserialize)]
struct CargoManifest {
    package: PackageMetadata,
}

#[derive(Deserialize)]
struct PackageMetadata {
    version: String,
    license: String,
    repository: String,
}

#[derive(Deserialize)]
struct ExtensionMetadata {
    version: String,
    id: String,
    repository: String,
    authors: Vec<String>,
    #[serde(default)]
    capabilities: Vec<Capability>,
}

/// A capability that `extension.toml` grants; Zed checks the URL of each download against it.
#[derive(Deserialize)]
struct Capability {
    kind: String,
    host: Option<String>,
    path: Option<Vec<String>>,
}

impl ExtensionMetadata {
    /// Whether the downloads that Zed allows are the release assets of `repository`,
    /// from which the extension builds the URL of the server.
    fn allows_release_downloads(&self) -> bool {
        let Some(("github.com", owner_name)) = self
            .repository
            .strip_prefix("https://")
            .and_then(|rest| rest.split_once('/'))
        else {
            return false;
        };
        let expected: Vec<_> = owner_name
            .split('/')
            .chain(["releases", "download", "**"])
            .collect();
        let downloads: Vec<_> = self
            .capabilities
            .iter()
            .filter(|c| c.kind == "download_file")
            .collect();
        downloads.len() == 1
            && downloads[0].host.as_deref() == Some("github.com")
            && downloads[0]
                .path
                .as_deref()
                .is_some_and(|path| path == expected)
    }
}

fn manifest<T: DeserializeOwned>(path: &str) -> Result<T> {
    toml::from_str(&fs::read_to_string(root().join(path))?)
        .with_context(|| format!("Invalid metadata in {path}"))
}

pub fn version() -> Result<String> {
    let native: CargoManifest = manifest("crates/translation-lsp/Cargo.toml")?;
    let extension: CargoManifest = manifest("zed-extension/Cargo.toml")?;
    let registry: ExtensionMetadata = manifest("zed-extension/extension.toml")?;
    let version = native.package.version;
    ensure!(
        extension.package.version == version && registry.version == version,
        "Release versions must match"
    );
    let parts: Vec<_> = version.split('.').collect();
    ensure!(
        parts.len() == 3
            && parts.iter().all(|p| !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p.len() == 1 || !p.starts_with('0'))),
        "Expected major.minor.patch"
    );
    Ok(version)
}

pub fn check() -> Result<()> {
    version()?;
    targets()?;
    let license = fs::read_to_string(root().join("LICENSE"))?.replace("\r\n", "\n");
    ensure!(
        license.starts_with("MIT License\n")
            && license
                == fs::read_to_string(root().join("zed-extension/LICENSE"))?.replace("\r\n", "\n"),
        "MIT licenses must match"
    );
    for path in [
        "crates/translation-lsp/Cargo.toml",
        "zed-extension/Cargo.toml",
    ] {
        let m: CargoManifest = manifest(path)?;
        ensure!(
            m.package.license == "MIT" && m.package.repository == REPOSITORY,
            "Invalid package metadata in {path}"
        );
    }
    let m: ExtensionMetadata = manifest("zed-extension/extension.toml")?;
    ensure!(
        m.id == "translation-lsp" && m.repository == REPOSITORY && !m.authors.is_empty(),
        "Invalid extension metadata"
    );
    ensure!(
        m.allows_release_downloads(),
        "The download_file capability of extension.toml must allow exactly the release downloads of its repository"
    );
    vscode::check_manifest(&version()?)?;
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub version: String,
    pub commit: String,
    pub dirty: bool,
    pub source_date_epoch: u64,
    pub target: String,
}

impl Source {
    pub fn current(target: &Target, allow_dirty: bool) -> Result<Self> {
        let dirty = !git(&["status", "--porcelain=v1", "--untracked-files=normal"])?.is_empty();
        ensure!(
            !dirty || allow_dirty,
            "Commit the source first, or use --allow-dirty for local checks"
        );
        Ok(Self {
            version: version()?,
            commit: git(&["rev-parse", "HEAD"])?,
            dirty,
            source_date_epoch: git(&["show", "-s", "--format=%ct", "HEAD"])?.parse()?,
            target: target.target.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
