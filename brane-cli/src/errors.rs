/* ERRORS.rs
 *   by Lut99
 *
 * Created:
 *   17 Feb 2022, 10:27:28
 * Last edited:
 *   21 Feb 2022, 12:46:23
 * Auto updated?
 *   Yes
 *
 * Description:
 *   File that contains file-spanning error definitions for the brane-cli
 *   package.
**/

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::path::PathBuf;

use crate::packages::PackageError;
use specifications::package::PackageKindError;
use specifications::container::ContainerInfoError;


/***** GLOBALS *****/
lazy_static! { static ref CLI_LINE_SEPARATOR: String = (0..80).map(|_| '-').collect::<String>(); }





/***** ERROR ENUMS *****/
/// Collects toplevel and uncategorized errors in the brane-cli package.
#[derive(Debug)]
pub enum CliError {
    // Main error kinds, split in their own enums
    /// Errors that occur during the build command
    BuildError{ err: BuildError },
    /// Errors that occur during the import command
    ImportError{ err: ImportError },
    /// Temporary wrapper around any anyhow error
    OtherError{ err: anyhow::Error },

    // A few miscellaneous, inter-subcommand errors
    /// Could not resolve the path to the package file
    PackageFileCanonicalizeError{ path: PathBuf, err: std::io::Error },
    /// Could not resolve the path to the context
    WorkdirCanonicalizeError{ path: PathBuf, err: std::io::Error },
    /// Could not resolve a string to a package kind
    IllegalPackageKind{ kind: String, err: PackageKindError },
    /// Could not open the main package file of the package to build.
    PackageFileOpenError{ file: PathBuf, err: std::io::Error },
    /// Could not read the main package file of the package to build.
    PackageFileReadError{ file: PathBuf, err: std::io::Error },
    /// Could not read from a given directory
    DirectoryReadError{ dir: PathBuf, err: std::io::Error },
    /// Could not automatically determine package file inside a directory.
    UndeterminedPackageFile{ dir: PathBuf },
    /// Could not automatically determine package kind based on the file.
    UndeterminedPackageKind{ file: PathBuf },
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            CliError::BuildError{ err }  => write!(f, "{}", err),
            CliError::ImportError{ err } => write!(f, "{}", err),
            CliError::OtherError{ err }  => write!(f, "{}", err),

            CliError::PackageFileCanonicalizeError{ path, err } => write!(f, "Could not resolve package file path '{}': {}", path.display(), err),
            CliError::WorkdirCanonicalizeError{ path, err }     => write!(f, "Could not resolve working directory '{}': {}", path.display(), err),
            CliError::IllegalPackageKind{ kind, err }           => write!(f, "Illegal package kind '{}': {}", kind, err),
            CliError::PackageFileOpenError{ file, err }         => write!(f, "Could not open package file '{}': {}", file.display(), err),
            CliError::PackageFileReadError{ file, err }         => write!(f, "Could not read from package file '{}': {}", file.display(), err),
            CliError::DirectoryReadError{ dir, err }            => write!(f, "Could not read from directory '{}': {}", dir.display(), err),
            CliError::UndeterminedPackageFile{ dir }            => write!(f, "Could not determine package file in directory '{}'; specify it manually with '--file'", dir.display()),
            CliError::UndeterminedPackageKind{ file }           => write!(f, "Could not determine package from package file '{}'; specify it manually with '--kind'", file.display()),
        }
    }
}

impl Error for CliError {}



/// Collects errors during the build subcommand
#[derive(Debug)]
pub enum BuildError {
    /// Could not open the given container info file
    ContainerInfoOpenError{ file: PathBuf, err: std::io::Error },
    /// Could not read/open the given container info file
    ContainerInfoParseError{ file: PathBuf, err: ContainerInfoError },
    /// Could not read/open the given OAS document
    OasDocumentParseError{ file: PathBuf, err: anyhow::Error },
    /// Could not properly convert the OpenAPI document into a PackageInfo
    PackageInfoFromOpenAPIError{ err: anyhow::Error },

    /// Could not write to the DockerFile string.
    DockerFileWriteError{ err: std::fmt::Error },
    /// A given filepath escaped the working directory
    UnsafePath{ path: String },
    /// The entrypoint executable referenced was not found
    MissingExecutable{ path: PathBuf },
    /// Could not create/resolve the package directory
    PackageDirError{ err: PackageError },

    /// A lock file exists for the current building package, so wait
    LockFileExists{ path: PathBuf },
    /// Could not create a file lock for system reasons
    LockCreateError{ path: PathBuf, err: std::io::Error },
    /// Could not create a file within the package directory
    PackageFileCreateError{ path: PathBuf, err: std::io::Error },
    /// Could not write to a file within the package directory
    PackageFileWriteError{ path: PathBuf, err: std::io::Error },
    /// Could not serialize the ContainerInfo back to text.
    ContainerInfoSerializeError{ err: serde_yaml::Error },
    /// Could not serialize the OpenAPI document back to text.
    OpenAPISerializeError{ err: serde_yaml::Error },
    /// Could not serialize the PackageInfo.
    PackageInfoSerializeError{ err: serde_yaml::Error },

    /// Could not resolve the custom branelet's path
    BraneletCanonicalizeError{ path: PathBuf, err: std::io::Error },
    /// Could not copy the branelet executable
    BraneletCopyError{ source: PathBuf, target: PathBuf, err: std::io::Error },

    /// Could not clear an existing working directory
    WdClearError{ path: PathBuf, err: std::io::Error },
    /// Could not create a new working directory
    WdCreateError{ path: PathBuf, err: std::io::Error },
    /// Could not canonicalize file's path that will be copied to the working directory
    WdSourceFileCanonicalizeError{ path: PathBuf, err: std::io::Error },
    /// Could not canonicalize a workdir file's path
    WdTargetFileCanonicalizeError{ path: PathBuf, err: std::io::Error },
    /// Could not create a directory in the working directory
    WdDirCreateError{ path: PathBuf, err: std::io::Error },
    /// Could not copy a file to the working directory
    WdFileCopyError{ source: PathBuf, target: PathBuf, err: std::io::Error },
    /// Could not copy a directory to the working directory
    WdDirCopyError{ source: PathBuf, target: PathBuf, err: fs_extra::error::Error },

    /// Could not launch the command to compress the working directory
    WdCompressionLaunchError{ command: String, err: std::io::Error },
    /// Command to compress the working directory returned a non-zero exit code
    WdCompressionError{ command: String, code: i32, stdout: String, stderr: String },
    /// Could not launch the command to remove the working directory
    WdRemoveLaunchError{ command: String, err: std::io::Error },
    /// Command to remove the working directory returned a non-zero exit code
    WdRemoveError{ command: String, code: i32, stdout: String, stderr: String },

    /// Failed to remove an existing build of this package/version from the docker daemon
    DockerCleanupError{ image: String, err: bollard::errors::Error },
    /// Failed to cleanup a file from the build directory after a successfull build.
    FileCleanupError{ path: PathBuf, err: std::io::Error },
    /// Failed to cleanup the .lock file from the build directory after a successfull build.
    LockCleanupError{ path: PathBuf, err: std::io::Error },
    /// Failed to cleanup the build directory after a failed build.
    CleanupError{ path: PathBuf, err: std::io::Error },

    /// Could not launch the command to see if buildkit is installed
    BuildKitLaunchError{ command: String, err: std::io::Error },
    /// The simple command to instantiate/test the BuildKit plugin for Docker returned a non-success
    BuildKitError{ command: String, code: i32, stdout: String, stderr: String },
    /// Could not launch the command to build the package image
    ImageBuildLaunchError{ command: String, err: std::io::Error },
    /// The command to build the image returned a non-zero exit code (we don't accept stdout or stderr here, as the command's output itself will be passed to stdout & stderr)
    ImageBuildError{ command: String, code: i32 },
}

impl Display for BuildError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            BuildError::ContainerInfoOpenError{ file, err }  => write!(f, "Could not open the container info file '{}': {}", file.display(), err),
            BuildError::ContainerInfoParseError{ file, err } => write!(f, "Could not parse the container info file '{}': {}", file.display(), err),
            BuildError::OasDocumentParseError{ file, err }   => write!(f, "Could not parse the OAS Document '{}': {}", file.display(), err),
            BuildError::PackageInfoFromOpenAPIError{ err }   => write!(f, "Could not convert the OAS Document into a Package Info file: {}", err),

            BuildError::DockerFileWriteError{ err }              => write!(f, "Could not write to the internal DockerFile: {}", err),
            BuildError::UnsafePath{ path }                       => write!(f, "File '{}' tries to escape package working directory; consider moving Brane's working directory up (using --workdir) and avoid '..'", path),
            BuildError::MissingExecutable{ path }                => write!(f, "Could not find the package entrypoint '{}'", path.display()),
            BuildError::PackageDirError{ err }                   => write!(f, "Could not create package directory: '{}'", err),

            BuildError::LockFileExists{ path }              => write!(f, "The build directory '{}' is busy; try again later (a lock file exists)", path.display()),
            BuildError::LockCreateError{ path, err }        => write!(f, "Could not create lock file '{}': {}", path.display(), err),
            BuildError::PackageFileCreateError{ path, err } => write!(f, "Could not create file '{}' within the package directory: {}", path.display(), err),
            BuildError::PackageFileWriteError{ path, err }  => write!(f, "Could not write to file '{}' within the package directory: {}", path.display(), err),
            BuildError::ContainerInfoSerializeError{ err }  => write!(f, "Could not re-serialize container.yml: {}", err),
            BuildError::OpenAPISerializeError{ err }        => write!(f, "Could not re-serialize OpenAPI document: {}", err),
            BuildError::PackageInfoSerializeError{ err }    => write!(f, "Could not serialize generated package info file: {}", err),

            BuildError::BraneletCanonicalizeError{ path, err }   => write!(f, "Could not resolve custom init binary path '{}': {}", path.display(), err),
            BuildError::BraneletCopyError{ source, target, err } => write!(f, "Could not copy custom init binary from '{}' to '{}': {}", source.display(), target.display(), err),

            BuildError::WdClearError{ path, err }                  => write!(f, "Could not clear existing package working directory '{}': {}", path.display(), err),
            BuildError::WdCreateError{ path, err }                 => write!(f, "Could not create package working directory '{}': {}", path.display(), err),
            BuildError::WdSourceFileCanonicalizeError{ path, err } => write!(f, "Could not resolve file '{}' in the package info file: {}", path.display(), err),
            BuildError::WdTargetFileCanonicalizeError{ path, err } => write!(f, "Could not resolve file '{}' in the package working directory: {}", path.display(), err),
            BuildError::WdDirCreateError{ path, err }              => write!(f, "Could not create directory '{}' in the package working directory: {}", path.display(), err),
            BuildError::WdFileCopyError{ source, target, err }     => write!(f, "Could not copy file '{}' to '{}' in the package working directory: {}", source.display(), target.display(), err),
            BuildError::WdDirCopyError{ source, target, err }      => write!(f, "Could not copy directory '{}' to '{}' in the package working directory: {}", source.display(), target.display(), err),

            BuildError::WdCompressionLaunchError{ command, err }            => write!(f, "Could not run command '{}' to compress working directory: {}", command, err),
            BuildError::WdCompressionError{ command, code, stdout, stderr } => write!(f, "Command '{}' to compress working directory returned exit code {}:\n\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n\n", command, code, *CLI_LINE_SEPARATOR, stdout, *CLI_LINE_SEPARATOR, *CLI_LINE_SEPARATOR, stderr, *CLI_LINE_SEPARATOR),
            BuildError::WdRemoveLaunchError{ command, err }                 => write!(f, "Could not run command '{}' to remove used working directory: {}", command, err),
            BuildError::WdRemoveError{ command, code, stdout, stderr }      => write!(f, "Command '{}' to remove used working directory returned exit code {}:\n\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n\n", command, code, *CLI_LINE_SEPARATOR, stdout, *CLI_LINE_SEPARATOR, *CLI_LINE_SEPARATOR, stderr,*CLI_LINE_SEPARATOR),

            BuildError::DockerCleanupError{ image, err } => write!(f, "Could not remove existing image '{}' from docker daemon: {}", image, err),
            BuildError::FileCleanupError{ path, err }    => write!(f, "Could not clean '{}' from build directory: {}", path.display(), err),
            BuildError::LockCleanupError{ path, err }    => write!(f, "Could not clean the lock file ('{}') from build directory: {}", path.display(), err),
            BuildError::CleanupError{ path, err }        => write!(f, "Could not clean build directory '{}': {}", path.display(), err),

            BuildError::BuildKitLaunchError{ command, err }            => write!(f, "Could not determine if Docker & BuildKit are installed: failed to run command '{}': {}", command, err),
            BuildError::BuildKitError{ command, code, stdout, stderr } => write!(f, "Could not run a Docker BuildKit (command '{}' returned exit code {}): is BuildKit installed?\n\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n\n", command, code, *CLI_LINE_SEPARATOR, stdout, *CLI_LINE_SEPARATOR, *CLI_LINE_SEPARATOR, stderr,*CLI_LINE_SEPARATOR),
            BuildError::ImageBuildLaunchError{ command, err }          => write!(f, "Could not run command '{}' to build the package image: {}", command, err),
            BuildError::ImageBuildError{ command, code }               => write!(f, "Command '{}' to build the package image returned exit code {}", command, code),
        }
    }
}

impl Error for BuildError {}



/// Collects errors during the import subcommand
#[derive(Debug)]
pub enum ImportError {
    /// Error for when we could not create a temporary directory
    TempDirError{ err: std::io::Error },
    /// Could not resolve the path to the temporary repository directory
    TempDirCanonicalizeError{ path: PathBuf, err: std::io::Error },
    /// Error for when we failed to clone a repository
    RepoCloneError{ repo: String, target: PathBuf, err: git2::Error },

    /// Error for when a path supposed to refer inside the repository escaped out of it
    RepoEscapeError{ path: PathBuf },
}

impl Display for ImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            ImportError::TempDirError{ err }                   => write!(f, "Could not create temporary repository directory: {}", err),
            ImportError::TempDirCanonicalizeError{ path, err } => write!(f, "Could not resolve temporary directory path '{}': {}", path.display(), err),
            ImportError::RepoCloneError{ repo, target, err }   => write!(f, "Could not clone repository at '{}' to directory '{}': {}", repo, target.display(), err),

            ImportError::RepoEscapeError{ path } => write!(f, "Path '{}' points outside of repository folder", path.display()),
        }
    }
}

impl Error for ImportError {}
