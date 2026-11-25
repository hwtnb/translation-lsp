//! The packages of the VS Code extension: one `.vsix` for each release target,
//! holding the server of that target's release archive.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Read, Write},
    path::{self, Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use zip::ZipArchive;

use crate::{
    archive, smoke,
    source::{self, Os, Source, Target},
};

const NPM: &str = if cfg!(windows) { "npm.cmd" } else { "npm" };

/// The publisher of the extension on the Marketplace.
pub const PUBLISHER: &str = "hwtnb";
/// The name of the extension on the Marketplace, under its publisher.
pub const NAME: &str = "translation-lsp";

/// The files of every package besides the server, as `vsce` names them.
const FILES: &[&str] = &[
    "extension.vsixmanifest",
    "[Content_Types].xml",
    "extension/LICENSE.txt",
    "extension/readme.md",
    "extension/images/icon.png",
    "extension/THIRD_PARTY_LICENSES.txt",
    "extension/package.json",
    "extension/package.nls.json",
    "extension/package.nls.ja.json",
    "extension/l10n/bundle.l10n.ja.json",
    "extension/out/extension.js",
];

/// The folder of the package that holds the server.
const SERVER: &str = "extension/server";

/// The most files that a package may hold before they are read; a package
/// holds 14.
const MAX_FILES: usize = 64;

/// The most bytes that the files of a package may hold once extracted, before
/// and while they are read. The package of 0.9.0 for macOS on ARM64 holds
/// 32 MB, almost all of it the server; this leaves room for larger servers
/// without reading whatever a damaged package declares.
const MAX_SIZE: u64 = 256 << 20;

/// The files of a release archive that a package holds in its server folder,
/// under the same names.
fn list_server_files(target: &Target) -> [&'static str; 3] {
    [target.executable(), "LICENSE", "THIRD_PARTY_LICENSES.txt"]
}

fn find_extension() -> PathBuf {
    source::root().join("vscode-extension")
}

/// The fields of `package.json` that name the extension and its version.
#[derive(Deserialize)]
struct Identity {
    publisher: String,
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct Repository {
    url: String,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(flatten)]
    identity: Identity,
    repository: Repository,
}

#[derive(Deserialize)]
struct LockedPackage {
    version: String,
}

#[derive(Deserialize)]
struct LockFile {
    version: String,
    packages: BTreeMap<String, LockedPackage>,
}

/// Checks that `package.json` and `package-lock.json` of the extension give
/// its publisher and name and the server's version, and `package.json` its
/// repository.
pub fn check_manifest(version: &str) -> Result<()> {
    let extension = find_extension();
    let manifest: Manifest = serde_json::from_slice(&fs::read(extension.join("package.json"))?)
        .context("Invalid vscode-extension/package.json")?;
    let lockfile: LockFile =
        serde_json::from_slice(&fs::read(extension.join("package-lock.json"))?)
            .context("Invalid vscode-extension/package-lock.json")?;
    check_metadata(&manifest, &lockfile, version)
}

fn check_metadata(manifest: &Manifest, lockfile: &LockFile, version: &str) -> Result<()> {
    check_identity(&manifest.identity, version).context("In vscode-extension/package.json")?;
    ensure!(
        lockfile.version == version
            && lockfile.packages.get("").map(|p| p.version.as_str()) == Some(version),
        "The version of the VS Code extension in package-lock.json must be {version}"
    );
    ensure!(
        manifest.repository.url == source::REPOSITORY,
        "The repository of the VS Code extension must be {}",
        source::REPOSITORY
    );
    Ok(())
}

fn check_identity(identity: &Identity, version: &str) -> Result<()> {
    ensure!(
        identity.publisher == PUBLISHER && identity.name == NAME && identity.version == version,
        "Expected the extension {PUBLISHER}.{NAME} {version}, not {}.{} {}",
        identity.publisher,
        identity.name,
        identity.version
    );
    Ok(())
}

/// The server folder of the extension, which `vsce` packages; it is removed
/// again when this is dropped.
struct ServerFolder(PathBuf);

impl ServerFolder {
    fn create() -> Result<Self> {
        let path = find_extension().join("server");
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for ServerFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Creates the package of `target` in `directory` from the release archive
/// there, checks it, and on this machine's target tests its server. An earlier
/// package of the same name is replaced only once the new one has passed.
pub fn package(target: &Target, directory: &Path) -> Result<PathBuf> {
    let version = source::version()?;
    let directory = path::absolute(directory)?;
    let archive = directory.join(target.archive_name(&version));
    let unpacked = tempfile::tempdir()?;
    let (_, source) = archive::unpack(&archive, unpacked.path())
        .with_context(|| format!("Reading {}", archive.display()))?;

    // `.vscodeignore` leaves the server out, so that a folder left by an
    // interrupted run never reaches a package for development.
    let ignore = unpacked.path().join("vscodeignore");
    let rules = fs::read_to_string(find_extension().join(".vscodeignore"))?;
    fs::write(&ignore, format!("{rules}\n!server/**\n"))?;
    // vsce deletes its output before writing it, so it writes into a folder
    // on the same file system, under the name that the checks expect.
    let staging = tempfile::Builder::new()
        .prefix(".vsix-")
        .tempdir_in(&directory)?;
    let staged = staging.path().join(target.vsix_name(&version));

    let server = ServerFolder::create()?;
    for name in list_server_files(target) {
        // The copy keeps the permission to run that `unpack` gave the executable.
        fs::copy(unpacked.path().join(name), server.0.join(name))?;
    }
    let status = Command::new(NPM)
        .args(["exec", "--no", "--", "vsce", "package", "--target"])
        .arg(target.vscode_target())
        .arg("--ignoreFile")
        .arg(&ignore)
        .arg("--out")
        .arg(&staged)
        // vsce then writes the files in order and with this time.
        .env("SOURCE_DATE_EPOCH", source.source_date_epoch.to_string())
        .current_dir(find_extension())
        .status()
        .context("Starting npm")?;
    drop(server);
    ensure!(
        status.success(),
        "vsce package failed for {}",
        target.target
    );

    check(&staged, &archive)?;
    if target.is_host() {
        smoke(&staged, target)?;
    }
    let output = directory.join(target.vsix_name(&version));
    fs::rename(&staged, &output)?;
    Ok(output)
}

/// A package opened for reading, with the index of each of its files by name.
struct Package {
    zip: ZipArchive<File>,
    files: BTreeMap<String, usize>,
}

impl Package {
    /// Opens the package at `path` and lists its files without reading them.
    fn open(path: &Path) -> Result<Self> {
        let zip = ZipArchive::new(File::open(path)?)
            .with_context(|| format!("Reading {}", path.display()))?;
        // The archive keeps only one of the files of a name, so the names
        // that it lists never repeat.
        ensure!(
            !zip.has_duplicate_names(),
            "{} names a file twice",
            path.display()
        );
        ensure!(
            zip.len() <= MAX_FILES,
            "{} holds more than {MAX_FILES} files",
            path.display()
        );
        let mut files = BTreeMap::new();
        let mut size = 0u64;
        for index in 0..zip.len() {
            let entry = zip.by_index_data(index)?;
            ensure!(entry.is_file(), "Expected only files in {}", path.display());
            size = size.saturating_add(entry.size());
            files.insert(entry.name()?.into_owned(), index);
        }
        ensure!(
            size <= MAX_SIZE,
            "{} holds more than {MAX_SIZE} bytes once extracted",
            path.display()
        );
        Ok(Self { zip, files })
    }

    fn index(&self, name: &str) -> Result<usize> {
        self.files
            .get(name)
            .copied()
            .with_context(|| format!("Missing {name}"))
    }

    /// The permissions that the package gives file `name` on Unix.
    fn mode(&self, name: &str) -> Result<Option<u32>> {
        Ok(self.zip.by_index_data(self.index(name)?)?.unix_mode())
    }

    /// Extracts file `name` to its end into `output`, and no more than the
    /// size that the package declares. Reaching the end checks its CRC.
    fn extract(&mut self, name: &str, output: &mut impl Write) -> Result<()> {
        let mut entry = self.zip.by_index(self.index(name)?)?;
        let size = entry.size();
        let copied = io::copy(&mut (&mut entry).take(size + 1), output)
            .with_context(|| format!("Extracting {name}"))?;
        ensure!(
            copied == size,
            "{name} does not hold the size that the package declares"
        );
        Ok(())
    }

    /// Reads file `name`, as `extract` extracts it.
    fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        self.extract(name, &mut data)?;
        Ok(data)
    }
}

/// The value of attribute `name` of the first `element` in `xml`.
fn find_attribute<'a>(xml: &'a str, element: &str, name: &str) -> Option<&'a str> {
    let start = xml.find(&format!("<{element} "))?;
    let element = &xml[start..start + xml[start..].find('>')?];
    let key = format!(" {name}=\"");
    let value = &element[element.find(&key)? + key.len()..];
    Some(&value[..value.find('"')?])
}

/// Checks that the XML manifest of a package names this extension, `version`,
/// and `target`, and marks no pre-release. The manifest is searched as text,
/// so it must hold one `Identity` and no comment, which could hold another.
fn check_vsix_manifest(xml: &str, target: &Target, version: &str) -> Result<()> {
    ensure!(
        !xml.contains("<!--") && xml.matches("<Identity").count() == 1,
        "The manifest must hold one Identity and no comment"
    );
    let identity = |name| find_attribute(xml, "Identity", name);
    ensure!(
        identity("Publisher") == Some(PUBLISHER)
            && identity("Id") == Some(NAME)
            && identity("Version") == Some(version)
            && identity("TargetPlatform") == Some(target.vscode_target()),
        "The manifest must give {PUBLISHER}.{NAME} {version} for target {}",
        target.vscode_target()
    );
    // vsce writes this property only for `--pre-release`. Its name alone
    // fails, however the manifest quotes it.
    ensure!(
        !xml.contains("Microsoft.VisualStudio.Code.PreRelease"),
        "The manifest marks a pre-release"
    );
    Ok(())
}

/// Checks the package at `path` against the release archive at `archive` it
/// was made from: its server and license files, and what `check_contents` checks.
pub fn check(path: &Path, archive: &Path) -> Result<()> {
    let files = archive::read(archive)?;
    let source: Source =
        serde_json::from_slice(files.get("SOURCE.json").context("Missing SOURCE.json")?)?;
    let target = source::target(&source.target)?;
    let mut package = check_contents(path, &target, &source.version)?;
    for file in list_server_files(&target) {
        ensure!(
            package.read(&format!("{SERVER}/{file}"))? == files[file],
            "{file} in {} differs from the release archive",
            target.vsix_name(&source.version)
        );
    }
    Ok(())
}

/// Checks the name and the files of the package of `target` at `path`, that
/// its server may be run on macOS and Linux, and that it is this extension of
/// this target and `version`. Returns the package.
fn check_contents(path: &Path, target: &Target, version: &str) -> Result<Package> {
    let name = target.vsix_name(version);
    ensure!(
        path.file_name().and_then(|n| n.to_str()) == Some(&name),
        "Expected the package of {} to be named {name}",
        target.target
    );

    let mut package = Package::open(path)?;
    let expected: BTreeSet<String> = FILES
        .iter()
        .map(|file| file.to_string())
        .chain(
            list_server_files(target)
                .iter()
                .map(|file| format!("{SERVER}/{file}")),
        )
        .collect();
    let actual: BTreeSet<String> = package.files.keys().cloned().collect();
    ensure!(
        actual == expected,
        "{name} holds other files than expected; missing: {:?}, unexpected: {:?}",
        expected.difference(&actual).collect::<Vec<_>>(),
        actual.difference(&expected).collect::<Vec<_>>()
    );
    // Files that nothing compares are extracted too, so that damaged data in
    // any of them fails.
    for file in &actual {
        package.extract(file, &mut io::sink())?;
    }

    if target.os != Os::Windows {
        let mode = package.mode(&format!("{SERVER}/{}", target.executable()))?;
        ensure!(
            mode.is_some_and(|mode| mode & 0o111 == 0o111),
            "The server in {name} may not be run once extracted: {}",
            mode.map_or("no Unix permissions".to_owned(), |mode| format!("{mode:o}"))
        );
    }

    let identity: Identity = serde_json::from_slice(&package.read("extension/package.json")?)
        .with_context(|| format!("Invalid package.json in {name}"))?;
    check_identity(&identity, version).with_context(|| format!("In package.json of {name}"))?;
    let xml = String::from_utf8(package.read("extension.vsixmanifest")?)?;
    check_vsix_manifest(&xml, target, version).with_context(|| format!("In {name}"))?;
    Ok(package)
}

/// Tests the server of the package at `path`, extracted with the permissions
/// that the package gives it, as `smoke` tests the server of an archive.
fn smoke(path: &Path, target: &Target) -> Result<()> {
    let mut package = Package::open(path)?;
    let server = format!("{SERVER}/{}", target.executable());
    let temporary = tempfile::tempdir()?;
    let binary = temporary.path().join(target.executable());
    fs::write(&binary, package.read(&server)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = package
            .mode(&server)?
            .context("The server has no Unix permissions")?;
        fs::set_permissions(&binary, fs::Permissions::from_mode(mode & 0o777))?;
    }
    smoke::run(&binary, &source::version()?)?;
    println!("Server of the VS Code package passed initialize, mock Hover, and shutdown");
    Ok(())
}

#[cfg(test)]
pub mod tests;
