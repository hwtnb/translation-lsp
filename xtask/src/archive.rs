use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use flate2::{Compression, GzBuilder, read::GzDecoder};
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, Header};
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    source::{self, Arch, Os, Source, Target},
    vscode,
};

pub type Files = BTreeMap<String, Vec<u8>>;

pub fn check_binary(path: &Path, target: &Target) -> Result<()> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header)?;
    let arm = target.arch == Arch::Aarch64;
    let (machine, expected) = match target.os {
        Os::Windows => {
            ensure!(&header[..2] == b"MZ", "Expected a Windows PE executable");
            file.seek(SeekFrom::Start(
                u32::from_le_bytes(header[60..64].try_into()?) as u64,
            ))?;
            let mut pe = [0; 6];
            file.read_exact(&mut pe)?;
            ensure!(&pe[..4] == b"PE\0\0", "Invalid PE header");
            (
                u16::from_le_bytes(pe[4..6].try_into()?) as u32,
                if arm { 0xaa64 } else { 0x8664 },
            )
        }
        Os::Linux => {
            ensure!(
                &header[..6] == b"\x7fELF\x02\x01",
                "Expected a little-endian 64-bit ELF executable"
            );
            (
                u16::from_le_bytes(header[18..20].try_into()?) as u32,
                if arm { 183 } else { 62 },
            )
        }
        Os::Mac => {
            ensure!(
                &header[..4] == b"\xcf\xfa\xed\xfe",
                "Expected a 64-bit Mach-O executable"
            );
            (
                u32::from_le_bytes(header[4..8].try_into()?),
                if arm { 0x0100000c } else { 0x01000007 },
            )
        }
    };
    ensure!(
        machine == expected,
        "Executable architecture does not match target"
    );
    Ok(())
}

fn write(path: &Path, files: &Files, executable: &str, epoch: u64) -> Result<()> {
    let file = File::create(path)?;
    if path.extension().is_some_and(|e| e == "zip") {
        let mut archive = ZipWriter::new(file);
        for (name, data) in files {
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .last_modified_time(DateTime::default())
                .unix_permissions(if name == executable { 0o755 } else { 0o644 });
            archive.start_file(name, options)?;
            archive.write_all(data)?;
        }
        archive.finish()?;
    } else {
        let compressed = GzBuilder::new()
            .mtime(epoch.try_into()?)
            .write(file, Compression::default());
        let mut archive = Builder::new(compressed);
        for (name, data) in files {
            let mut header = Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mtime(epoch);
            header.set_mode(if name == executable { 0o755 } else { 0o644 });
            header.set_cksum();
            archive.append_data(&mut header, name, data.as_slice())?;
        }
        archive.into_inner()?.finish()?;
    }
    Ok(())
}

pub fn read(path: &Path) -> Result<Files> {
    let mut files = Files::new();
    let file = File::open(path)?;
    if path.extension().is_some_and(|e| e == "zip") {
        let mut archive = ZipArchive::new(file)?;
        // The archive keeps only one of the files of a name, so the check
        // below never sees a name twice.
        ensure!(!archive.has_duplicate_names(), "Duplicate archive entry");
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            ensure!(
                entry.is_file() && !entry.is_symlink(),
                "Expected regular archive files"
            );
            let name = entry.name()?.into_owned();
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            ensure!(
                files.insert(name, data).is_none(),
                "Duplicate archive entry"
            );
        }
    } else {
        let mut archive = Archive::new(GzDecoder::new(file));
        for entry in archive.entries()? {
            let mut entry = entry?;
            ensure!(
                entry.header().entry_type().is_file(),
                "Expected regular archive files"
            );
            let name = entry
                .path()?
                .to_str()
                .context("Non-UTF8 archive path")?
                .to_owned();
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            ensure!(
                files.insert(name, data).is_none(),
                "Duplicate archive entry"
            );
        }
    }
    Ok(files)
}

pub fn digest(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn package(
    target: &Target,
    binary: &Path,
    output: &Path,
    source: &Source,
    notices: Vec<u8>,
) -> Result<PathBuf> {
    check_binary(binary, target)?;
    ensure!(
        source.target == target.target,
        "Source target must match archive"
    );
    fs::create_dir_all(output)?;
    let archive = output.join(target.archive_name(&source.version));
    let files = Files::from([
        (target.executable().into(), fs::read(binary)?),
        ("LICENSE".into(), fs::read(source::root().join("LICENSE"))?),
        ("THIRD_PARTY_LICENSES.txt".into(), notices),
        ("SOURCE.json".into(), serde_json::to_vec_pretty(source)?),
        (
            "README.txt".into(),
            format!(
                concat!(
                    "Translation LSP {}\nTarget: {}\n\n",
                    "Extract and set lsp.translation-lsp.binary.path to the absolute path of {} ",
                    "for manual installation. Keep the license files.\n",
                    "Default mock mode contacts no provider and translates only bundled samples.\n",
                    "Settings and privacy: {}\n"
                ),
                source.version,
                target.target,
                target.executable(),
                source::REPOSITORY
            )
            .into_bytes(),
        ),
    ]);
    write(
        &archive,
        &files,
        target.executable(),
        source.source_date_epoch,
    )?;
    let name = archive
        .file_name()
        .context("Archive filename")?
        .to_str()
        .context("Archive filename encoding")?;
    fs::write(
        output.join(format!("{name}.sha256")),
        format!("{}  {name}\n", digest(&archive)?),
    )?;
    Ok(archive)
}

pub fn unpack(path: &Path, directory: &Path) -> Result<(PathBuf, Source)> {
    let files = read(path)?;
    let source: Source =
        serde_json::from_slice(files.get("SOURCE.json").context("Missing SOURCE.json")?)?;
    let target = source::target(&source.target)?;
    let expected: BTreeSet<_> = [
        target.executable(),
        "LICENSE",
        "THIRD_PARTY_LICENSES.txt",
        "SOURCE.json",
        "README.txt",
    ]
    .into_iter()
    .collect();
    ensure!(
        files.keys().map(String::as_str).collect::<BTreeSet<_>>() == expected,
        "Unexpected archive contents"
    );
    ensure!(
        path.file_name().and_then(|n| n.to_str()) == Some(&target.archive_name(&source.version))
            && source.version == source::version()?,
        "Archive name and version must match source"
    );
    for (name, data) in files {
        fs::write(directory.join(name), data)?;
    }
    let binary = directory.join(target.executable());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    }
    check_binary(&binary, &target)?;
    Ok((binary, source))
}

/// The names of the release archives and the VS Code packages of all
/// release targets, in the order that `SHA256SUMS` lists them.
fn list_assets(version: &str) -> Result<Vec<String>> {
    let targets = source::targets()?;
    let archives: BTreeSet<_> = targets.iter().map(|t| t.archive_name(version)).collect();
    Ok(archives
        .into_iter()
        .chain(targets.iter().map(|t| t.vsix_name(version)))
        .collect())
}

/// The digest of each asset in `directory`, by name, in the order of `list_assets`.
fn digest_assets(directory: &Path, version: &str) -> Result<Vec<(String, String)>> {
    list_assets(version)?
        .into_iter()
        .map(|name| {
            let sum = digest(&directory.join(&name)).with_context(|| format!("Reading {name}"))?;
            Ok((name, sum))
        })
        .collect()
}

/// Checks that `directory` holds the release archives and the VS Code packages
/// of all release targets and no other archive or package.
fn check_names(directory: &Path, version: &str) -> Result<()> {
    let expected: BTreeSet<_> = list_assets(version)?.into_iter().collect();
    let actual: BTreeSet<_> = fs::read_dir(directory)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .filter(|n| n.ends_with(".zip") || n.ends_with(".tar.gz") || n.ends_with(".vsix"))
        .collect();
    ensure!(
        actual == expected,
        "Expected exactly the six release archives and the six VS Code packages; missing: {:?}, unexpected: {:?}",
        expected.difference(&actual).collect::<Vec<_>>(),
        actual.difference(&expected).collect::<Vec<_>>()
    );
    Ok(())
}

/// Checks that each archive in `directory` was built from clean source at
/// `commit`, and that each package holds the server of its archive.
fn check_builds(directory: &Path, version: &str, commit: &str) -> Result<()> {
    for target in source::targets()? {
        let name = target.archive_name(version);
        let archive = directory.join(&name);
        let temporary = tempfile::tempdir()?;
        let (_, source) = unpack(&archive, temporary.path())?;
        ensure!(
            !source.dirty && source.commit == commit,
            "{name} must be built from clean source at commit {commit}"
        );
        vscode::check(&directory.join(target.vsix_name(version)), &archive)?;
    }
    Ok(())
}

/// Checks the archives and the VS Code packages of all release targets in
/// `directory`, and writes `SHA256SUMS` for them.
pub fn checksums(directory: &Path, commit: &str) -> Result<()> {
    let version = source::version()?;
    check_names(directory, &version)?;
    let sums = digest_assets(directory, &version)?;
    // Each archive must match the checksum that `package` wrote next to it
    // before it is opened.
    for (name, sum) in sums.iter().filter(|(name, _)| !name.ends_with(".vsix")) {
        ensure!(
            fs::read_to_string(directory.join(format!("{name}.sha256")))?.replace("\r\n", "\n")
                == format!("{sum}  {name}\n"),
            "Checksum mismatch for {name}"
        );
    }
    check_builds(directory, &version, commit)?;
    let text: String = sums
        .iter()
        .map(|(name, sum)| format!("{sum}  {name}\n"))
        .collect();
    fs::write(directory.join("SHA256SUMS"), text)?;
    Ok(())
}

/// The digests that a `SHA256SUMS` file lists, by name. Fails for a line
/// without a digest and a name, and for a name listed twice.
fn read_sums(path: &Path) -> Result<BTreeMap<String, String>> {
    let text = fs::read_to_string(path).with_context(|| format!("Reading {}", path.display()))?;
    let mut sums = BTreeMap::new();
    for line in text.lines() {
        let (sum, name) = line
            .split_once("  ")
            .with_context(|| format!("Invalid line in SHA256SUMS: {line:?}"))?;
        ensure!(
            sums.insert(name.to_owned(), sum.to_owned()).is_none(),
            "SHA256SUMS lists {name} twice"
        );
    }
    Ok(sums)
}

/// Checks the assets that were downloaded from the release of `tag` into
/// `directory`, before its VS Code packages are published: that the release's
/// `SHA256SUMS` lists exactly the assets of the tag's version, that each asset
/// matches it, and then what `checksums` checks, against `commit`.
pub fn check_release(directory: &Path, tag: &str, commit: &str) -> Result<()> {
    let version = source::version()?;
    ensure!(
        tag == format!("v{version}"),
        "The tag {tag} does not name version {version} of the checkout"
    );
    let listed = read_sums(&directory.join("SHA256SUMS"))?;
    let expected: BTreeSet<_> = list_assets(&version)?.into_iter().collect();
    let names: BTreeSet<_> = listed.keys().cloned().collect();
    ensure!(
        names == expected,
        "SHA256SUMS must list the assets of version {version}; missing: {:?}, unexpected: {:?}",
        expected.difference(&names).collect::<Vec<_>>(),
        names.difference(&expected).collect::<Vec<_>>()
    );
    check_names(directory, &version)?;
    for (name, sum) in digest_assets(directory, &version)? {
        ensure!(listed[&name] == sum, "{name} does not match SHA256SUMS");
    }
    check_builds(directory, &version, commit)?;
    println!("The assets of {tag} match SHA256SUMS and the checkout");
    Ok(())
}

#[cfg(test)]
pub mod tests;
