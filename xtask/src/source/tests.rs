use super::*;

/// The metadata of an `extension.toml` with this repository and these capabilities.
fn parse(repository: &str, capabilities: &str) -> ExtensionMetadata {
    toml::from_str(&format!(
        "id = \"translation-lsp\"\nversion = \"1.0.0\"\nauthors = [\"a\"]\nrepository = \"{repository}\"\n{capabilities}"
    ))
    .unwrap()
}

const DOWNLOAD: &str = "[[capabilities]]\nkind = \"download_file\"\nhost = \"github.com\"\n";

#[test]
fn zed_allows_the_downloads_of_the_repository_that_the_extension_downloads_from() -> Result<()> {
    let repository = "https://github.com/owner/name";
    let path = "path = [\"owner\", \"name\", \"releases\", \"download\", \"**\"]\n";
    assert!(parse(repository, &format!("{DOWNLOAD}{path}")).allows_release_downloads());
    assert!(
        parse(
            repository,
            &format!(
                "[[capabilities]]\nkind = \"process:exec\"\ncommand = \"x\"\nargs = []\n{DOWNLOAD}{path}"
            )
        )
        .allows_release_downloads()
    );

    for capabilities in [
        String::new(),
        format!("{DOWNLOAD}path = [\"owner\", \"old-name\", \"releases\", \"download\", \"**\"]\n"),
        format!("{DOWNLOAD}path = [\"owner\", \"name\", \"releases\", \"**\"]\n"),
        format!("{DOWNLOAD}path = [\"owner\", \"name\", \"**\"]\n"),
        format!("[[capabilities]]\nkind = \"download_file\"\nhost = \"example.com\"\n{path}"),
        format!("{DOWNLOAD}{path}{DOWNLOAD}path = [\"**\"]\n"),
    ] {
        assert!(
            !parse(repository, &capabilities).allows_release_downloads(),
            "{capabilities}"
        );
    }
    assert!(
        !parse(
            "https://example.com/owner/name",
            &format!("{DOWNLOAD}{path}")
        )
        .allows_release_downloads()
    );

    let manifest: ExtensionMetadata = manifest("zed-extension/extension.toml")?;
    assert!(manifest.allows_release_downloads());
    Ok(())
}
