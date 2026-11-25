use std::{env, path::PathBuf, slice};

use anyhow::{Context, Result, ensure};
use clap::Parser;

mod archive;
mod notices;
mod smoke;
mod source;
mod vscode;

/// Packages the native server and checks what a release is made from.
#[derive(Parser)]
enum Command {
    /// Checks that the versions, licenses, and release targets agree.
    CheckSource,
    /// Prints the release targets as a GitHub Actions matrix.
    Matrix,
    /// Prints the version that the manifests agree on.
    Version,
    /// Generates the dependency notices of all release targets and discards them.
    CheckNotices,
    /// Creates an archive and tests its extracted executable in mock mode.
    Package {
        /// One of the release targets that `matrix` prints.
        #[arg(long, value_name = "TRIPLE")]
        target: String,
        /// Defaults to the release build of the target.
        #[arg(long, value_name = "PATH")]
        binary: Option<PathBuf>,
        /// Defaults to `dist` in the repository.
        #[arg(long, value_name = "DIRECTORY")]
        output: Option<PathBuf>,
        /// Marks an archive of uncommitted source, which `checksums` rejects.
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Tests the extracted executable of an archive in mock mode.
    Smoke {
        #[arg(long, value_name = "PATH")]
        archive: PathBuf,
    },
    /// Creates and checks the VS Code extension packages from the release
    /// archives, and tests the server of the package for this machine.
    PackageVscode {
        /// One of the release targets that `matrix` prints; may be repeated.
        /// Defaults to all of them.
        #[arg(long, value_name = "TRIPLE")]
        target: Vec<String>,
        /// Holds the archives and receives the packages. Defaults to `dist`
        /// in the repository.
        #[arg(long, value_name = "PATH")]
        directory: Option<PathBuf>,
    },
    /// Checks the assets downloaded from a published release against its
    /// `SHA256SUMS` and, as `checksums` does, against the checkout of its
    /// tag, before its VS Code packages are published.
    CheckRelease {
        #[arg(long, value_name = "PATH")]
        directory: PathBuf,
        /// The tag of the release, such as `v1.2.3`.
        #[arg(long)]
        tag: String,
    },
    /// Checks the archives and VS Code packages of all release targets and
    /// writes `SHA256SUMS`.
    Checksums {
        /// Defaults to `dist` in the repository.
        #[arg(long, value_name = "PATH")]
        directory: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let root = source::root();
    match Command::parse() {
        Command::CheckSource => {
            source::check()?;
            println!("Release source metadata is consistent");
        }
        Command::Matrix => {
            source::check()?;
            println!("{}", source::matrix()?);
        }
        Command::Version => println!("{}", source::version()?),
        Command::CheckNotices => {
            let targets = source::targets()?;
            notices::generate(&targets)?;
            println!("Dependency notices can be generated for all release targets");
        }
        Command::Package {
            target,
            binary,
            output,
            allow_dirty,
        } => {
            let target = source::target(&target)?;
            let binary = binary.unwrap_or_else(|| {
                env::var_os("CARGO_TARGET_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| root.join("target"))
                    .join(&target.target)
                    .join("release")
                    .join(target.executable())
            });
            let output = output.unwrap_or_else(|| root.join("dist"));
            source::check()?;
            let source = source::Source::current(&target, allow_dirty)?;
            let notices = notices::generate(slice::from_ref(&target))?;
            let path = archive::package(&target, &binary, &output, &source, notices)?;
            smoke::archive(&path)?;
            println!("{}", path.display());
        }
        Command::Smoke { archive } => smoke::archive(&archive)?,
        Command::PackageVscode { target, directory } => {
            source::check()?;
            let directory = directory.unwrap_or_else(|| root.join("dist"));
            let targets = if target.is_empty() {
                source::targets()?
            } else {
                target
                    .iter()
                    .map(|name| source::target(name))
                    .collect::<Result<_>>()?
            };
            let version = source::version()?;
            let missing: Vec<_> = targets
                .iter()
                .map(|t| t.archive_name(&version))
                .filter(|name| !directory.join(name).is_file())
                .collect();
            ensure!(
                missing.is_empty(),
                "Missing release archives in {}: {missing:?}",
                directory.display()
            );
            for target in targets {
                println!("{}", vscode::package(&target, &directory)?.display());
            }
        }
        Command::CheckRelease { directory, tag } => {
            let commit = source::git(&["rev-parse", "HEAD"])?;
            let tagged = source::git(&[
                "rev-parse",
                "--verify",
                &format!("refs/tags/{tag}^{{commit}}"),
            ])
            .with_context(|| format!("Reading the commit of the tag {tag}"))?;
            ensure!(
                tagged == commit,
                "The checkout is not the commit of the tag {tag}"
            );
            archive::check_release(&directory, &tag, &commit)?;
        }
        Command::Checksums { directory } => {
            let directory = directory.unwrap_or_else(|| root.join("dist"));
            archive::checksums(&directory, &source::git(&["rev-parse", "HEAD"])?)?;
        }
    }
    Ok(())
}
