#[macro_use]
extern crate human_panic;

use std::path::PathBuf;
use std::process;
use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use git2::Repository;
use log::LevelFilter;
use tempfile::tempdir;

use brane_cli::{build_ecu, build_oas, packages, registry, repl, run, test, version};
use brane_cli::errors::{CliError, ImportError};
use specifications::package::PackageKind;
use specifications::version::Version;


#[derive(Parser)]
#[clap(name = "brane", about = "The Brane command-line interface.")]
struct Cli {
    #[clap(short, long, help = "Enable debug mode")]
    debug: bool,
    #[clap(short, long, help = "Skip dependencies check")]
    skip_check: bool,
    #[clap(subcommand)]
    sub_command: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    #[clap(name = "build", about = "Build a package")]
    Build {
        #[clap(short, long, help = "Path to the directory to use as container working directory (defaults to the folder of the package file itself)")]
        workdir: Option<PathBuf>,
        #[clap(name = "FILE", help = "Path to the file to build")]
        file: PathBuf,
        #[clap(short, long, help = "Kind of package: cwl, dsl, ecu or oas")]
        kind: Option<String>,
        #[clap(short, long, help = "Path to the init binary to use (override Brane's binary)")]
        init: Option<PathBuf>,
        #[clap(long, help = "Don't delete build files")]
        keep_files: bool,
    },

    #[clap(name = "import", about = "Import a package")]
    Import {
        #[clap(name = "REPO", help = "Name of the GitHub repository containing the package")]
        repo: String,
        #[clap(short, long, help = "Path to the directory to use as container working directory, relative to the repository (defaults to the folder of the package file itself)")]
        workdir: Option<PathBuf>,
        #[clap(name = "FILE", help = "Path to the file to build, relative to the repository")]
        file: Option<PathBuf>,
        #[clap(short, long, help = "Kind of package: cwl, dsl, ecu or oas")]
        kind: Option<String>,
        #[clap(short, long, help = "Path to the init binary to use (override Brane's binary)")]
        init: Option<PathBuf>,
    },

    #[clap(name = "inspect", about = "Inspect a package")]
    Inspect {
        #[clap(name = "NAME", help = "Name of the package")]
        name: String,
        #[clap(name = "VERSION", default_value = "latest", help = "Version of the package")]
        version: Version,
    },

    #[clap(name = "list", about = "List packages")]
    List {
        #[clap(short, long, help = "If given, only print the latest version of each package instead of all versions")]
        latest: bool,
    },

    #[clap(name = "load", about = "Load a package locally")]
    Load {
        #[clap(name = "NAME", help = "Name of the package")]
        name: String,
        #[clap(short, long, default_value = "latest", help = "Version of the package")]
        version: Version,
    },

    #[clap(name = "login", about = "Log in to a registry")]
    Login {
        #[clap(name = "HOST", help = "Hostname of the registry")]
        host: String,
        #[clap(short, long, help = "Username of the account")]
        username: String,
    },

    #[clap(name = "logout", about = "Log out from a registry")]
    Logout {},

    #[clap(name = "pull", about = "Pull a package from a registry")]
    Pull {
        #[clap(name = "NAME", help = "Name of the package")]
        name: String,
        #[clap(name = "VERSION", default_value = "latest", help = "Version of the package")]
        version: Version,
    },

    #[clap(name = "push", about = "Push a package to a registry")]
    Push {
        #[clap(name = "NAME", help = "Name of the package")]
        name: String,
        #[clap(name = "VERSION", default_value = "latest", help = "Version of the package")]
        version: Version,
    },

    #[clap(name = "remove", about = "Remove a local package.")]
    Remove {
        #[clap(name = "NAME", help = "Name of the package.")]
        name: String,
        /* TIM */
        // #[clap(short, long, help = "Version of the package")]
        #[clap(name = "VERSION", help = "Version of the package. If omitted, removes ALL versions of this package.")]
        /*******/
        version: Option<Version>,
        #[clap(short, long, help = "Don't ask for confirmation.")]
        force: bool,
    },

    #[clap(name = "repl", about = "Start an interactive DSL session")]
    Repl {
        #[clap(short, long, help = "Use Bakery instead of BraneScript")]
        bakery: bool,
        #[clap(short, long, help = "Clear history before session")]
        clear: bool,
        #[clap(short, long, value_names = &["address[:port]"], help = "Create a remote REPL session")]
        remote: Option<String>,
        #[clap(short, long, value_names = &["uid"], help = "Attach to an existing remote session")]
        attach: Option<String>,
        #[clap(short, long, help = "The directory to mount as /data")]
        data: Option<PathBuf>,
    },

    #[clap(name = "run", about = "Run a DSL script locally")]
    Run {
        #[clap(name = "FILE", help = "Path to the file to run")]
        file: PathBuf,
        #[clap(short, long, help = "The directory to mount as /data")]
        data: Option<PathBuf>,
    },

    #[clap(name = "test", about = "Test a package locally")]
    Test {
        #[clap(name = "NAME", help = "Name of the package")]
        name: String,
        #[clap(short, long, default_value = "latest", help = "Version of the package")]
        version: Version,
        #[clap(short, long, help = "The directory to mount as /data")]
        data: Option<PathBuf>,
    },

    #[clap(name = "search", about = "Search a registry for packages")]
    Search {
        #[clap(name = "TERM", help = "Term to use as search criteria")]
        term: Option<String>,
    },

    #[clap(name = "unpublish", about = "Remove a package from a registry")]
    Unpublish {
        #[clap(name = "NAME", help = "Name of the package")]
        name: String,
        #[clap(name = "VERSION", help = "Version of the package")]
        version: Version,
        #[clap(short, long, help = "Don't ask for confirmation")]
        force: bool,
    },

    #[clap(name = "version", about = "Shows the version number for this Brane CLI tool and (if logged in) the remote Driver.")]
    Version {
        #[clap(short, long, help = "If given, shows the local version in an easy-to-be-parsed format. Note that, if given in combination with '--remote', this one is always reported first.")]
        local: bool,
        #[clap(short, long, help = "If given, shows the remote Driver version in an easy-to-be-parsed format. Note that, if given in combination with '--local', this one is always reported second.")]
        remote: bool,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse the CLI arguments
    dotenv().ok();
    let options = Cli::parse();

    // Prepare the logger
    let mut logger = env_logger::builder();
    logger.format_module_path(false);

    if options.debug {
        logger.filter_module("brane", LevelFilter::Debug).init();
    } else {
        logger.filter_module("brane", LevelFilter::Info).init();

        setup_panic!(Metadata {
            name: "Brane CLI".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: env!("CARGO_PKG_AUTHORS").replace(":", ", ").into(),
            homepage: env!("CARGO_PKG_HOMEPAGE").into(),
        });
    }

    // Check dependencies if not withheld from doing so
    if !options.skip_check {
        match brane_cli::utils::check_dependencies().await {
            Ok(Ok(()))   => {},
            Ok(Err(err)) => { eprintln!("Dependencies not met: {}", err); process::exit(1); }
            Err(err)     => { eprintln!("Could not check for dependencies: {}", err); process::exit(1); }
        }
    }

    // Run the subcommand given
    match run(options).await {
        Ok(_) => process::exit(0),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}

/// **Edited: now returning CliErrors.**
/// 
/// Runs one of the subcommand as given on the Cli.
/// 
/// **Arguments**
///  * `options`: The struct with (parsed) Cli-options and subcommands.
/// 
/// **Returns**  
/// Nothing if the subcommand executed successfully (they are self-contained), or a CliError otherwise.
async fn run(options: Cli) -> Result<(), CliError> {
    use SubCommand::*;
    match options.sub_command {
        Build {
            workdir,
            file,
            kind,
            init,
            keep_files,
        } => {
            // Resolve the working directory
            let workdir = match workdir {
                Some(workdir) => workdir,
                None          => match std::fs::canonicalize(&file) {
                    Ok(file) => file.parent().unwrap().to_path_buf(),
                    Err(err) => { return Err(CliError::PackageFileCanonicalizeError{ path: file, err }); }
                },
            };
            let workdir = match std::fs::canonicalize(workdir) {
                Ok(workdir) => workdir,
                Err(err)    => { return Err(CliError::WorkdirCanonicalizeError{ path: file, err }); }
            };

            // Resolve the kind of the file
            let kind = if let Some(kind) = kind {
                match PackageKind::from_str(&kind) {
                    Ok(kind) => kind,
                    Err(err) => { return Err(CliError::IllegalPackageKind{ kind, err }); }
                }
            } else {
                match brane_cli::utils::determine_kind(&file) {
                    Ok(kind) => kind,
                    Err(err) => { return Err(CliError::UtilError{ err }); }
                }
            };

            // Build a new package with it
            match kind {
                PackageKind::Ecu => build_ecu::handle(workdir, file, init, keep_files).await.map_err(|err| CliError::BuildError{ err })?,
                PackageKind::Oas => build_oas::handle(workdir, file, init, keep_files).await.map_err(|err| CliError::BuildError{ err })?,
                _                => eprintln!("Unsupported package kind: {}", kind),
            }
        }
        Import {
            repo,
            workdir,
            file,
            kind,
            init,
        } => {
            // Prepare the input URL and output directory
            let url = format!("https://github.com/{}", repo);
            let dir = match tempdir() {
                Ok(dir)  => dir,
                Err(err) => { return Err(CliError::ImportError{ err: ImportError::TempDirError{ err } }); }
            };
            let dir_path = match std::fs::canonicalize(dir.path()) {
                Ok(dir_path) => dir_path,
                Err(err)     => { return Err(CliError::ImportError{ err: ImportError::TempDirCanonicalizeError{ path: dir.path().to_path_buf(), err } }); }
            };

            // Pull the repository
            if let Err(err) = Repository::clone(&url, &dir_path) {
                return Err(CliError::ImportError{ err: ImportError::RepoCloneError{ repo: url, target: dir_path, err } });
            };

            // Try to get which file we need to use as package file
            let file = match file {
                Some(file) => dir_path.join(file),
                None       => dir_path.join(brane_cli::utils::determine_file(&dir_path).map_err(|err| CliError::UtilError{ err })?),
            };
            let file = match std::fs::canonicalize(&file) {
                Ok(file) => file,
                Err(err) => { return Err(CliError::PackageFileCanonicalizeError{ path: file, err }); }
            };
            if !file.starts_with(&dir_path) { return Err(CliError::ImportError{ err: ImportError::RepoEscapeError{ path: file } }); }

            // Try to resolve the working directory relative to the repository
            let workdir = match workdir {
                Some(workdir) => dir.path().join(workdir),
                None          => file.parent().unwrap().to_path_buf(),
            };
            let workdir = match std::fs::canonicalize(workdir) {
                Ok(workdir) => workdir,
                Err(err)    => { return Err(CliError::WorkdirCanonicalizeError{ path: file, err }); }
            };
            if !workdir.starts_with(&dir_path) { return Err(CliError::ImportError{ err: ImportError::RepoEscapeError{ path: file } }); }

            // Resolve the kind of the file
            let kind = if let Some(kind) = kind {
                match PackageKind::from_str(&kind) {
                    Ok(kind) => kind,
                    Err(err) => { return Err(CliError::IllegalPackageKind{ kind, err }); }
                }
            } else {
                match brane_cli::utils::determine_kind(&file) {
                    Ok(kind) => kind,
                    Err(err) => { return Err(CliError::UtilError{ err }); }
                }
            };

            // Build a new package with it
            match kind {
                PackageKind::Ecu => build_ecu::handle(workdir, file, init, false).await.map_err(|err| CliError::BuildError{ err })?,
                PackageKind::Oas => build_oas::handle(workdir, file, init, false).await.map_err(|err| CliError::BuildError{ err })?,
                _                => eprintln!("Unsupported package kind: {}", kind),
            }
        }

        Inspect { name, version } => {
            if let Err(err) = packages::inspect(name, version) { return Err(CliError::OtherError{ err }); };
        }
        List { latest } => {
            if let Err(err) = packages::list(latest) { return Err(CliError::OtherError{ err: anyhow::anyhow!(err) }); };
        }
        Load { name, version } => {
            if let Err(err) = packages::load(name, version).await { return Err(CliError::OtherError{ err }); };
        }
        Login { host, username } => {
            if let Err(err) = registry::login(host, username) { return Err(CliError::OtherError{ err }); };
        }
        Logout {} => {
            if let Err(err) = registry::logout() { return Err(CliError::OtherError{ err }); };
        }
        Pull { name, version } => {
            if let Err(err) = registry::pull(name, version).await { return Err(CliError::OtherError{ err }); };
        }
        Push { name, version } => {
            if let Err(err) = registry::push(name, version).await { return Err(CliError::OtherError{ err }); };
        }
        Remove { name, version, force } => {
            if let Err(err) = packages::remove(name, version, force).await { return Err(CliError::OtherError{ err }); };
        }
        Repl {
            bakery,
            clear,
            remote,
            attach,
            data,
        } => {
            if let Err(err) = repl::start(bakery, clear, remote, attach, data).await { return Err(CliError::ReplError{ err }); };
        }
        Run { file, data } => {
            if let Err(err) = run::handle(file, data).await { return Err(CliError::OtherError{ err }); };
        }
        Test { name, version, data } => {
            if let Err(err) = test::handle(name, version, data).await { return Err(CliError::OtherError{ err }); };
        }
        Search { term } => {
            if let Err(err) = registry::search(term).await { return Err(CliError::OtherError{ err }); };
        }
        Unpublish { name, version, force } => {
            if let Err(err) = registry::unpublish(name, version, force).await { return Err(CliError::OtherError{ err }); };
        }
        Version { local, remote } => {
            if local || remote {
                // If any of local or remote is given, do those
                if local  { if let Err(err) = version::handle_local()        { return Err(CliError::VersionError{ err }); } }
                if remote { if let Err(err) = version::handle_remote().await { return Err(CliError::VersionError{ err }); } }

            } else {
                // Print neatly
                if let Err(err) = version::handle().await { return Err(CliError::VersionError{ err }); }
            }
        }
    }

    Ok(())
}
