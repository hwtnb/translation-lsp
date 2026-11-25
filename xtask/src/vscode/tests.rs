use std::io::Write;

use zip::{System, ZipWriter, write::SimpleFileOptions};

use super::*;
use crate::archive::tests::{binary, source};

/// A file of a package to write, with its permissions on Unix.
type Files = BTreeMap<String, (Vec<u8>, u32)>;

/// The files that `vsce` would package for the release archive at `archive`.
fn list_package_files(archive: &Path) -> Result<Files> {
    let files = archive::read(archive)?;
    let source: Source = serde_json::from_slice(&files["SOURCE.json"])?;
    let target = source::target(&source.target)?;
    let mut package: Files = FILES
        .iter()
        .map(|name| (name.to_string(), (b"file".to_vec(), 0o644)))
        .collect();
    package.insert(
        "extension/package.json".into(),
        (
            serde_json::to_vec(&serde_json::json!({
                "name": NAME,
                "publisher": PUBLISHER,
                "version": source.version,
            }))?,
            0o644,
        ),
    );
    package.insert(
        "extension.vsixmanifest".into(),
        (
            format!(
                "<PackageManifest><Metadata><Identity Language=\"en-US\" Id=\"{NAME}\" Version=\"{}\" Publisher=\"{PUBLISHER}\" TargetPlatform=\"{}\"/><Properties><Property Id=\"Microsoft.VisualStudio.Code.ExecutesCode\" Value=\"true\" /></Properties></Metadata></PackageManifest>",
                source.version,
                target.vscode_target()
            )
            .into_bytes(),
            0o644,
        ),
    );
    for name in list_server_files(&target) {
        let mode = if name == target.executable() {
            0o755
        } else {
            0o644
        };
        package.insert(format!("{SERVER}/{name}"), (files[name].clone(), mode));
    }
    Ok(package)
}

fn write_files(path: &Path, files: &Files) -> Result<()> {
    let mut package = ZipWriter::new(File::create(path)?);
    for (name, (data, mode)) in files {
        // As vsce writes it on macOS and Linux; on Windows the archive would
        // otherwise give its files no Unix permissions.
        let options = SimpleFileOptions::default()
            .system(System::Unix)
            .unix_permissions(*mode);
        package.start_file(name, options)?;
        package.write_all(data)?;
    }
    package.finish()?;
    Ok(())
}

/// Writes the package that `vsce` would make from the release archive at
/// `archive`, next to it.
pub fn write_package(archive: &Path) -> Result<PathBuf> {
    let source: Source = serde_json::from_slice(&archive::read(archive)?["SOURCE.json"])?;
    let target = source::target(&source.target)?;
    let path = archive.with_file_name(target.vsix_name(&source.version));
    write_files(&path, &list_package_files(archive)?)?;
    Ok(path)
}

/// Writes the release archive of `target` into `directory`.
fn write_archive(directory: &Path, target: &Target) -> Result<PathBuf> {
    let input = directory.join(target.executable());
    fs::write(&input, binary(target))?;
    archive::package(
        target,
        &input,
        directory,
        &source(target),
        b"license".to_vec(),
    )
}

#[test]
fn each_release_target_has_its_own_vsce_target() -> Result<()> {
    let names: BTreeMap<_, _> = source::targets()?
        .iter()
        .map(|t| (t.target.clone(), t.vscode_target()))
        .collect();
    assert_eq!(
        names,
        BTreeMap::from([
            ("aarch64-apple-darwin".to_owned(), "darwin-arm64"),
            ("x86_64-apple-darwin".to_owned(), "darwin-x64"),
            ("aarch64-unknown-linux-gnu".to_owned(), "linux-arm64"),
            ("x86_64-unknown-linux-gnu".to_owned(), "linux-x64"),
            ("aarch64-pc-windows-msvc".to_owned(), "win32-arm64"),
            ("x86_64-pc-windows-msvc".to_owned(), "win32-x64"),
        ])
    );
    let version = source::version()?;
    assert_eq!(
        source::target("x86_64-pc-windows-msvc")?.vsix_name(&version),
        format!("translation-lsp-v{version}-win32-x64.vsix")
    );
    Ok(())
}

#[test]
fn a_package_holds_the_server_of_its_archive_and_nothing_else() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    for target in source::targets()? {
        let archive = write_archive(temporary.path(), &target)?;
        let path = write_package(&archive)?;
        check(&path, &archive)?;

        let original = list_package_files(&archive)?;
        let rejected = |edit: &dyn Fn(&mut Files)| -> Result<String> {
            let mut files = list_package_files(&archive)?;
            edit(&mut files);
            write_files(&path, &files)?;
            let error = format!("{:#}", check(&path, &archive).unwrap_err());
            write_files(&path, &original)?;
            Ok(error)
        };
        let server = format!("{SERVER}/{}", target.executable());

        for extra in [
            "extension/src/extension.ts".to_owned(),
            "extension/node_modules/semver/index.js".to_owned(),
            format!("{SERVER}/translation-lsp.exe"),
            format!("{SERVER}/translation-lsp"),
        ] {
            if extra == server {
                continue;
            }
            let error = rejected(&|files| {
                files.insert(extra.clone(), (b"extra".to_vec(), 0o644));
            })?;
            assert!(error.contains("unexpected"), "{extra}: {error}");
        }
        let error = rejected(&|files| {
            files.remove("extension/THIRD_PARTY_LICENSES.txt");
        })?;
        assert!(error.contains("missing"), "{error}");
        let error = rejected(&|files| {
            files.get_mut(&server).unwrap().0.push(0);
        })?;
        assert!(error.contains("differs"), "{error}");
        let error = rejected(&|files| {
            files
                .get_mut(&format!("{SERVER}/THIRD_PARTY_LICENSES.txt"))
                .unwrap()
                .0 = b"other".to_vec();
        })?;
        assert!(error.contains("differs"), "{error}");
        let current = source::version()?;
        for (publisher, name, version) in [
            (PUBLISHER, NAME, "0.0.1"),
            ("other", NAME, current.as_str()),
            (PUBLISHER, "other", current.as_str()),
        ] {
            let error = rejected(&|files| {
                files.get_mut("extension/package.json").unwrap().0 = serde_json::to_vec(
                    &serde_json::json!({ "name": name, "publisher": publisher, "version": version }),
                )
                .unwrap();
            })?;
            assert!(error.contains("package.json"), "{error}");
        }
        for (from, to, message) in [
            (target.vscode_target(), "linux-armhf", "must give"),
            ("Id=\"translation-lsp\"", "Id=\"other\"", "must give"),
            ("Publisher=\"hwtnb\"", "Publisher=\"other\"", "must give"),
            // A comment that a search of the text would take for the Identity.
            (
                "<Metadata>",
                "<Metadata><!-- <Identity Id=\"translation-lsp\"/> -->",
                "one Identity",
            ),
            (
                "<Metadata>",
                "<Metadata><Identity Id=\"other\"/>",
                "one Identity",
            ),
            (
                "<Properties>",
                "<Properties><Property Id=\"Microsoft.VisualStudio.Code.PreRelease\" Value=\"true\" />",
                "pre-release",
            ),
            (
                "<Properties>",
                "<Properties><Property Id='Microsoft.VisualStudio.Code.PreRelease' Value='true' />",
                "pre-release",
            ),
        ] {
            let error = rejected(&|files| {
                let manifest = &mut files.get_mut("extension.vsixmanifest").unwrap().0;
                *manifest = String::from_utf8(manifest.clone())
                    .unwrap()
                    .replacen(from, to, 1)
                    .into_bytes();
            })?;
            assert!(error.contains(message), "{to}: {error}");
        }

        if target.os == Os::Windows {
            // Windows does not read the permissions.
            let mut files = original.clone();
            files.get_mut(&server).unwrap().1 = 0o644;
            write_files(&path, &files)?;
            check(&path, &archive)?;
            write_files(&path, &original)?;
        } else {
            let error = rejected(&|files| files.get_mut(&server).unwrap().1 = 0o644)?;
            assert!(error.contains("may not be run"), "{error}");
        }

        let renamed = path.with_file_name("translation-lsp.vsix");
        fs::rename(&path, &renamed)?;
        assert!(check(&renamed, &archive).is_err());
        fs::remove_file(renamed)?;
    }
    Ok(())
}

/// Sets the size that the central directory of the package at `path` gives
/// its file `name`, which `write_files` stores without zip64 fields.
fn declare_size(path: &Path, name: &str, size: u32) -> Result<()> {
    let mut bytes = fs::read(path)?;
    let start = bytes
        .windows(name.len())
        .rposition(|window| window == name.as_bytes())
        .unwrap();
    // The name follows the 46 bytes of a central directory header, which
    // holds the size from its 24th byte.
    let header = start - 46;
    assert_eq!(&bytes[header..header + 4], b"PK\x01\x02");
    bytes[header + 24..header + 28].copy_from_slice(&size.to_le_bytes());
    fs::write(path, bytes)?;
    Ok(())
}

#[test]
fn damaged_data_fails_in_a_file_that_nothing_compares() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let target = source::target("aarch64-apple-darwin")?;
    let archive = write_archive(temporary.path(), &target)?;
    let path = write_package(&archive)?;
    let mut files = list_package_files(&archive)?;
    let name = "extension/out/extension.js";
    files.get_mut(name).unwrap().0 = "console.log(1);\n".repeat(256).into_bytes();
    write_files(&path, &files)?;
    check(&path, &archive)?;

    // The name first follows the 30 bytes of its local header, which holds
    // the compressed size from its 18th byte and the length of the extra
    // field from its 28th; the compressed data follows both.
    let mut bytes = fs::read(&path)?;
    let start = bytes
        .windows(name.len())
        .position(|window| window == name.as_bytes())
        .unwrap();
    let header = start - 30;
    assert_eq!(&bytes[header..header + 4], b"PK\x03\x04");
    let compressed = u32::from_le_bytes(bytes[header + 18..header + 22].try_into()?) as usize;
    let extra = u16::from_le_bytes(bytes[header + 28..header + 30].try_into()?) as usize;
    assert!(compressed > 4);
    bytes[start + name.len() + extra + compressed / 2] ^= 0xff;
    fs::write(&path, bytes)?;
    let error = format!("{:#}", check(&path, &archive).unwrap_err());
    assert!(error.contains(name), "{error}");
    Ok(())
}

#[test]
fn a_package_is_rejected_before_its_files_are_read_when_it_cannot_be_one() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let target = source::target("x86_64-unknown-linux-gnu")?;
    let archive = write_archive(temporary.path(), &target)?;
    let path = write_package(&archive)?;
    let original = list_package_files(&archive)?;
    let error = |files: &Files, patch: &dyn Fn() -> Result<()>| -> Result<String> {
        write_files(&path, files)?;
        patch()?;
        Ok(format!("{:#}", check(&path, &archive).unwrap_err()))
    };

    // Two names of the same length, the second turned into the first.
    let mut files = original.clone();
    files.insert("extension/twice-1".into(), (b"one".to_vec(), 0o644));
    files.insert("extension/twice-2".into(), (b"two".to_vec(), 0o644));
    let twice = error(&files, &|| {
        let mut bytes = fs::read(&path)?;
        let found: Vec<_> = bytes
            .windows(17)
            .enumerate()
            .filter(|(_, window)| *window == b"extension/twice-2")
            .map(|(index, _)| index)
            .collect();
        for index in found {
            bytes[index + 16] = b'1';
        }
        fs::write(&path, bytes)?;
        Ok(())
    })?;
    assert!(twice.contains("names a file twice"), "{twice}");

    let mut files = original.clone();
    for index in 0..MAX_FILES {
        files.insert(format!("extension/extra-{index}"), (Vec::new(), 0o644));
    }
    let many = error(&files, &|| Ok(()))?;
    assert!(many.contains("more than"), "{many}");

    let large = error(&original, &|| {
        declare_size(&path, "extension/readme.md", MAX_SIZE as u32 + 1)
    })?;
    assert!(large.contains("bytes once extracted"), "{large}");
    let short = error(&original, &|| {
        declare_size(&path, "extension/package.json", 2)
    })?;
    assert!(short.contains("declare"), "{short}");
    Ok(())
}

#[test]
fn the_extension_manifest_gives_the_identity_version_and_repository_of_the_server() -> Result<()> {
    let version = source::version()?;
    check_manifest(&version)?;
    let manifest = |publisher: &str, name: &str, version: &str, url: &str| Manifest {
        identity: Identity {
            publisher: publisher.into(),
            name: name.into(),
            version: version.into(),
        },
        repository: Repository { url: url.into() },
    };
    let lockfile = |top: &str, root: &str| LockFile {
        version: top.into(),
        packages: BTreeMap::from([(
            String::new(),
            LockedPackage {
                version: root.into(),
            },
        )]),
    };
    let repository = source::REPOSITORY;
    check_metadata(
        &manifest(PUBLISHER, NAME, "1.2.3", repository),
        &lockfile("1.2.3", "1.2.3"),
        "1.2.3",
    )?;
    for (manifest, lockfile) in [
        (
            manifest(PUBLISHER, NAME, "1.2.4", repository),
            lockfile("1.2.3", "1.2.3"),
        ),
        (
            manifest(PUBLISHER, NAME, "1.2.3", repository),
            lockfile("1.2.4", "1.2.3"),
        ),
        (
            manifest(PUBLISHER, NAME, "1.2.3", repository),
            lockfile("1.2.3", "1.2.4"),
        ),
        (
            manifest("other", NAME, "1.2.3", repository),
            lockfile("1.2.3", "1.2.3"),
        ),
        (
            manifest(PUBLISHER, "other", "1.2.3", repository),
            lockfile("1.2.3", "1.2.3"),
        ),
        (
            manifest(
                PUBLISHER,
                NAME,
                "1.2.3",
                "https://github.com/hwtnb/other-repository",
            ),
            lockfile("1.2.3", "1.2.3"),
        ),
    ] {
        assert!(check_metadata(&manifest, &lockfile, "1.2.3").is_err());
    }
    Ok(())
}
