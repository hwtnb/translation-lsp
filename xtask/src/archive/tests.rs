use tempfile::NamedTempFile;

use super::*;
use crate::vscode::tests::write_package;

#[test]
fn checksum_matches_the_sha256_known_vector() -> Result<()> {
    let file = NamedTempFile::new()?;
    fs::write(file.path(), b"abc")?;
    assert_eq!(
        digest(file.path())?,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    Ok(())
}

/// Bytes that `check_binary` takes for an executable of `target`.
pub fn binary(target: &Target) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    let arm = target.arch == Arch::Aarch64;
    match target.os {
        Os::Mac => {
            bytes[..4].copy_from_slice(b"\xcf\xfa\xed\xfe");
            bytes[4..8]
                .copy_from_slice(&(if arm { 0x0100000cu32 } else { 0x01000007 }).to_le_bytes());
        }
        Os::Linux => {
            bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
            bytes[18..20].copy_from_slice(&(if arm { 183u16 } else { 62 }).to_le_bytes());
        }
        Os::Windows => {
            bytes[..2].copy_from_slice(b"MZ");
            bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
            bytes[64..68].copy_from_slice(b"PE\0\0");
            bytes[68..70].copy_from_slice(&(if arm { 0xaa64u16 } else { 0x8664 }).to_le_bytes());
        }
    }
    bytes
}

/// The source of a clean archive of `target` from commit `aaaa…`.
pub fn source(target: &Target) -> Source {
    Source {
        version: source::version().unwrap(),
        commit: "a".repeat(40),
        dirty: false,
        source_date_epoch: 1700000000,
        target: target.target.clone(),
    }
}

#[test]
fn metadata_and_all_six_archives_match_their_executables() -> Result<()> {
    source::check()?;
    let temporary = tempfile::tempdir()?;
    for target in source::targets()? {
        let input = temporary.path().join(target.executable());
        fs::write(&input, binary(&target))?;
        let path = package(
            &target,
            &input,
            temporary.path(),
            &source(&target),
            b"license".to_vec(),
        )?;
        let original = fs::read(&path)?;
        package(
            &target,
            &input,
            temporary.path(),
            &source(&target),
            b"license".to_vec(),
        )?;
        assert_eq!(original, fs::read(&path)?);
        let extracted = tempfile::tempdir()?;
        let (actual, info) = unpack(&path, extracted.path())?;
        assert_eq!(fs::read(&actual)?, binary(&target));
        assert_eq!(info.target, target.target);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&actual)?.permissions().mode() & 0o777, 0o755);
        }
        let wrong = source::targets()?
            .into_iter()
            .find(|t| t.os == target.os && t.arch != target.arch)
            .unwrap();
        assert!(check_binary(&actual, &wrong).is_err());
    }
    Ok(())
}

#[test]
fn unexpected_paths_are_rejected_before_writing() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let target = source::target("aarch64-pc-windows-msvc")?;
    let path = temporary
        .path()
        .join(target.archive_name(&source::version()?));
    let files = Files::from([
        ("../outside".into(), b"bad".to_vec()),
        ("SOURCE.json".into(), serde_json::to_vec(&source(&target))?),
    ]);
    write(&path, &files, target.executable(), 1700000000)?;
    let extracted = temporary.path().join("extracted");
    fs::create_dir(&extracted)?;
    assert!(unpack(&path, &extracted).is_err());
    assert_eq!(fs::read_dir(extracted)?.count(), 0);
    assert!(!temporary.path().join("outside").exists());
    Ok(())
}

/// Writes the clean archives of all release targets from commit `aaaa…`
/// into `directory`, with the packages that `vsce` would make from them
/// if `packages`.
fn write_release(directory: &Path, packages: bool) -> Result<()> {
    for target in source::targets()? {
        let input = directory.join(target.executable());
        fs::write(&input, binary(&target))?;
        let archive = package(
            &target,
            &input,
            directory,
            &source(&target),
            b"license".to_vec(),
        )?;
        if packages {
            write_package(&archive)?;
        }
    }
    Ok(())
}

#[test]
fn collection_rejects_damage_dirty_sources_wrong_commits_and_missing_targets_or_packages()
-> Result<()> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path();
    write_release(directory, false)?;
    let commit = "a".repeat(40);
    assert!(checksums(directory, &commit).is_err());
    write_release(directory, true)?;
    checksums(directory, &commit)?;
    assert_eq!(
        fs::read_to_string(directory.join("SHA256SUMS"))?
            .lines()
            .count(),
        12
    );
    let vsix = directory.join(source::targets()?[2].vsix_name(&source::version()?));
    let original = fs::read(&vsix)?;
    fs::write(&vsix, b"damaged")?;
    assert!(checksums(directory, &commit).is_err());
    fs::write(&vsix, original)?;
    assert!(checksums(directory, &"b".repeat(40)).is_err());
    let target = source::targets()?.remove(0);
    let archive = directory.join(target.archive_name(&source::version()?));
    let original = fs::read(&archive)?;
    fs::write(&archive, b"damaged")?;
    // Its checksum fails it before it is opened.
    let error = checksums(directory, &commit).unwrap_err().to_string();
    assert!(error.contains("Checksum mismatch"), "{error}");
    fs::write(&archive, original)?;
    let input = directory.join(target.executable());
    fs::write(&input, binary(&target))?;
    let mut dirty = source(&target);
    dirty.dirty = true;
    package(&target, &input, directory, &dirty, b"license".to_vec())?;
    assert!(checksums(directory, &commit).is_err());
    fs::remove_file(archive)?;
    assert!(checksums(directory, &commit).is_err());
    Ok(())
}

#[test]
fn a_release_must_match_its_checksums_its_tag_and_the_checkout_before_publishing() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path();
    write_release(directory, true)?;
    let commit = "a".repeat(40);
    checksums(directory, &commit)?;
    let tag = format!("v{}", source::version()?);
    check_release(directory, &tag, &commit)?;
    assert!(check_release(directory, "v0.0.1", &commit).is_err());
    let error = check_release(directory, &tag, &"b".repeat(40)).unwrap_err();
    assert!(error.to_string().contains("clean source"), "{error}");

    // A package of the same version from another build, listed with its
    // own digest, is told by the commit of its archive.
    let target = source::targets()?.remove(0);
    let other = tempfile::tempdir()?;
    let input = other.path().join(target.executable());
    fs::write(&input, binary(&target))?;
    let mut built = source(&target);
    built.commit = "c".repeat(40);
    let archive = package(&target, &input, other.path(), &built, b"license".to_vec())?;
    let vsix = write_package(&archive)?;
    let sums = directory.join("SHA256SUMS");
    let original = fs::read_to_string(&sums)?;
    let replace = |name: &str, from: &Path| -> Result<()> {
        let listed = original
            .lines()
            .find(|line| line.ends_with(name))
            .unwrap()
            .to_owned();
        let text =
            fs::read_to_string(&sums)?.replace(&listed, &format!("{}  {name}", digest(from)?));
        fs::write(&sums, text)?;
        fs::copy(from, directory.join(name))?;
        Ok(())
    };
    let backup = tempfile::tempdir()?;
    for path in [&archive, &vsix] {
        let name = path.file_name().unwrap();
        fs::copy(directory.join(name), backup.path().join(name))?;
        replace(name.to_str().unwrap(), path)?;
    }
    let error = check_release(directory, &tag, &commit).unwrap_err();
    assert!(error.to_string().contains("clean source"), "{error}");
    for path in [&archive, &vsix] {
        let name = path.file_name().unwrap();
        fs::copy(backup.path().join(name), directory.join(name))?;
    }
    fs::write(&sums, &original)?;
    check_release(directory, &tag, &commit)?;

    let package = directory.join(target.vsix_name(&source::version()?));
    let unchanged = fs::read(&package)?;
    let mut changed = unchanged.clone();
    changed.push(0);
    fs::write(&package, &changed)?;
    let error = check_release(directory, &tag, &commit).unwrap_err();
    assert!(error.to_string().contains("SHA256SUMS"), "{error}");
    fs::remove_file(&package)?;
    assert!(check_release(directory, &tag, &commit).is_err());
    fs::write(&package, &unchanged)?;

    let first = original.lines().next().unwrap();
    for text in [
        original.replacen(&format!("{first}\n"), "", 1),
        format!("{original}{first}\n"),
        format!("{original}{}  other.vsix\n", "0".repeat(64)),
    ] {
        fs::write(&sums, text)?;
        assert!(check_release(directory, &tag, &commit).is_err());
    }
    fs::write(&sums, format!("{original}{first}\n"))?;
    let error = check_release(directory, &tag, &commit).unwrap_err();
    assert!(error.to_string().contains("twice"), "{error}");
    Ok(())
}
