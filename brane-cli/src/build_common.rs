/* BUILD COMMON.rs
 *   by Lut99
 *
 * Created:
 *   21 Feb 2022, 12:32:28
 * Last edited:
 *   28 Mar 2022, 11:31:00
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains common macros, constants and functions between the different
 *   package kinds.
**/

use std::fs::{self, File};
use std::path::Path;
use std::process::Command;

use crate::errors::BuildError;


/***** COMMON MACROS *****/
/// Wrapper around write! that returns BuildErrors instead of standard format errors.
macro_rules! write_build {
    ($($e:expr),*) => {
        write!($($e),*).map_err(|err| BuildError::DockerfileStrWriteError{ err })
    }
}

/// Wrapper around writeln! that returns BuildErrors instead of standard format errors.
macro_rules! writeln_build {
    ($($e:expr),*) => {
        writeln!($($e),*).map_err(|err| BuildError::DockerfileStrWriteError{ err })
    }
}





/***** COMMON CONSTANTS */
/// The URL which we use to pull the latest branelet executable from.
pub const BRANELET_URL: &str = concat!(
    "https://github.com/epi-project/brane/releases/download/",
    concat!("v", env!("CARGO_PKG_VERSION")),
    "/branelet"
);

/// The URL Which we use to pull the latest JuiceFS executable from.
pub const JUICE_URL: &str =
    "https://github.com/juicedata/juicefs/releases/download/v0.12.1/juicefs-0.12.1-linux-amd64.tar.gz";





/***** COMMON FUNCTIONS *****/
/// **Edited: now returning BuildErrors. Also leaving .lock removal to the main handle function.**
/// 
/// Cleans the resulting build directory from the build files (but only if the build files should be removed).
/// 
/// **Arguments**
///  * `package_dir`: The directory to clean (we assume this has been canonicalized and thus exists).
///  * `files`: The files to remove from the build directory.
/// 
/// **Returns**  
/// Nothing - although this function will print BuildErrors as warnings to stderr using the logger.
pub fn clean_directory(
    package_dir: &Path,
    files: Vec<&str>,
) {
    // Remove the build files
    for file in files {
        let file = package_dir.join(file);
        if file.is_file() {
            if let Err(err) = fs::remove_file(&file) {
                warn!("{}", BuildError::FileCleanupError{ path: file, err });
            }
        } else if file.is_dir() {
            if let Err(err) = fs::remove_dir_all(&file) {
                warn!("{}", BuildError::DirCleanupError{ path: file, err });
            }
        } else {
            warn!("To-be-cleaned file '{}' is neither a file nor a directory", file.display());
        }
    }
}



/// Creates a new lock file in the given package directory.
/// 
/// **Arguments**
///  * `package_dir`: The directory to create the lock file in. We assume this path is already canonicalized and thus exists.
/// 
/// **Returns**  
/// Nothing if we created the .lock file successfully, or a BuildError otherwise (if the directory is already locked, for example).
pub fn lock_directory(
    package_dir: &Path,
) -> Result<(), BuildError> {
    debug!("Using package directory: '{}'", package_dir.display());

    // Make sure there is no lock file
    let lock = package_dir.join(".lock");
    if lock.exists() {
        return Err(BuildError::LockFileExists{ path: package_dir.to_path_buf() });
    }

    // Create the lock file
    if let Err(err) = File::create(&lock) {
        return Err(BuildError::LockCreateError{ path: lock, err });
    };

    // Success!
    Ok(())
}

/// Removes the lock file in the given package directory.
/// 
/// **Arguments**
///  * `package_dir`: The directory to remove the lock file from. We assume this path is already canonicalized and thus exists.
/// 
/// **Returns**  
/// Nothing - but does write a warning to stderr if we could not remove the lock.
pub fn unlock_directory(
    package_dir: &Path,
) {
    let lock = package_dir.join(".lock");
    if let Err(err) = fs::remove_file(&lock) {
        warn!("{}", BuildError::LockCleanupError{ path: lock, err });
    }
}



/// **Edited: now returning BuildErrors.**
/// 
/// Builds the docker image in the given package directory.
/// 
/// **Generic types**
///  * `P`: The Path-like type of the container directory path.
/// 
/// **Arguments**
///  * `package_dir`: The build directory for this image. We expect the actual image files to be under ./container.
///  * `tag`: Tag to give to the image so we can find it later (probably just <package name>:<package version>)
/// 
/// **Returns**  
/// Nothing if the image was build successfully, or a BuildError otherwise.
pub fn build_docker_image<P: AsRef<Path>>(
    package_dir : P,
    tag         : String,
) -> Result<(), BuildError> {
    // Prepare the command to check for buildx (and launch the buildx image, presumably)
    let mut command = Command::new("docker");
    command.arg("buildx");
    let buildx = match command.output() {
        Ok(buildx) => buildx,
        Err(err)   => { return Err(BuildError::BuildKitLaunchError{ command: format!("{:?}", command), err }); }
    };
    // Check if it was successfull
    if !buildx.status.success() {
        return Err(BuildError::BuildKitError{ command: format!("{:?}", command), code: buildx.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&buildx.stdout).to_string(), stderr: String::from_utf8_lossy(&buildx.stdout).to_string() });
    }

    // Next, launch the command to actually build the image
    let mut command = Command::new("docker");
    command.arg("buildx");
    command.arg("build");
    command.arg("--output");
    command.arg("type=docker,dest=image.tar");
    command.arg("--tag");
    command.arg(tag);
    command.arg(".");
    command.current_dir(package_dir);
    let output = match command.status() {
        Ok(output) => output,
        Err(err)   => { return Err(BuildError::ImageBuildLaunchError{ command: format!("{:?}", command), err }); }
    };
    // Check if it was successfull
    if !output.success() {
        return Err(BuildError::ImageBuildError{ command: format!("{:?}", command), code: output.code().unwrap_or(-1) });
    }

    // Done! :D
    Ok(())
}
