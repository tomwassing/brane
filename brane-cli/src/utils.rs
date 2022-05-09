/* UTILS.rs
 *   by Lut99
 *
 * Created:
 *   21 Feb 2022, 14:43:30
 * Last edited:
 *   28 Mar 2022, 12:10:00
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains useful utilities used throughout the brane-cli package.
**/

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

use bollard::Docker;

use specifications::package::PackageKind;
use specifications::version::Version;

use crate::{MIN_DOCKER_VERSION, MIN_BUILDX_VERSION};
use crate::errors::UtilError;


/***** HELPER ENUMS *****/
/// If a dependency is not met, this enum lists which one and why not.
#[derive(Debug)]
pub enum DependencyError {
    /// Docker cannot be reached
    DockerNotInstalled,
    /// Docker has a too low version
    DockerMinNotMet{ got: Version, expected: Version },

    /// The Buildkit plugin is not installed for Docker
    BuildkitNotInstalled,
    /// The Buildkit plugin has an incorrect version
    BuildKitMinNotMet{ got: Version, expected: Version },
}

impl Display for DependencyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            DependencyError::DockerNotInstalled               => write!(f, "Local Docker instance cannot be reached (is Docker installed and running?)"),
            DependencyError::DockerMinNotMet{ got, expected } => write!(f, "Docker version is {}, but Brane requires version {} or later", got, expected),

            DependencyError::BuildkitNotInstalled               => write!(f, "Local Docker instance does not have the Buildkit plugin installed"),
            DependencyError::BuildKitMinNotMet{ got, expected } => write!(f, "Buildkit plugin for Docker version is {}, but Brane requires version {} or later", got, expected),
        }
    }
}

impl Error for DependencyError {}





/***** UTILITIES *****/
/// **Edited: Now returning UtilErrors.**
/// 
/// Checks the runtime dependencies of brane-cli (Docker + BuildKit)
/// 
/// **Returns**  
/// Nothing if the dependencies are met, a DependencyError if it wasn't, or a UtilError if we couldn't determine.
pub async fn check_dependencies() -> Result<Result<(), DependencyError>, UtilError> {
    /* Docker */
    // Connect to the local instance using bollard
    let docker = match Docker::connect_with_local_defaults() {
        Ok(docker) => docker,
        Err(_)     => { return Ok(Err(DependencyError::DockerNotInstalled)); }
    };

    // Get the version of information of the docker container
    let docker_version = match docker.version().await {
        Ok(docker_version) => match docker_version.version {
            Some(docker_version) => docker_version,
            None                 => { return Err(UtilError::DockerNoVersion); }
        },
        Err(err)           => { return Err(UtilError::DockerVersionError{ err }); }
    };

    // Try to convert the version number to a semver
    let docker_version = match Version::from_str(&docker_version) {
        Ok(docker_version) => docker_version,
        Err(err)           => { return Err(UtilError::IllegalDockerVersion{ version: docker_version, err }); }
    };

    // Compare it with the required instance
    if docker_version < MIN_DOCKER_VERSION {
        return Ok(Err(DependencyError::DockerMinNotMet{ got: docker_version, expected: MIN_DOCKER_VERSION }));
    }



    /* Buildx */
    // // Run a command to get the buildx version
    // let mut command = Command::new("docker");
    // command.arg("buildx");
    // command.arg("version");
    // command.stdout(Stdio::piped());
    // let output = match command.output() {
    //     Ok(output) => output,
    //     Err(err)   => { return Err(UtilError::BuildxLaunchError{ command: format!("{:?}", command), err }); }
    // };
    // if !output.status.success() {
    //     return Ok(Err(DependencyError::BuildkitNotInstalled));
    // }
    // let buildx_version = String::from_utf8_lossy(&output.stdout).to_string();

    // // Get the second when splitting on spaces
    // let buildx_version = match buildx_version.split(' ').nth(1) {
    //     Some(buildx_version) => buildx_version,
    //     None                 => { return Err(UtilError::BuildxVersionNoParts{ version: buildx_version }); }
    // };

    // // Remove the first v
    // let buildx_version = if !buildx_version.is_empty() && buildx_version.starts_with('v') {
    //     &buildx_version[1..]
    // } else {
    //     return Err(UtilError::BuildxVersionNoV{ version: buildx_version.to_string() });
    // };

    // // Parse the first part up to a dash
    // let buildx_version = match buildx_version.find('-') {
    //     Some(dash_pos) => buildx_version[..dash_pos].to_string(),
    //     None           => { return Err(UtilError::BuildxVersionNoDash{ version: buildx_version.to_string() }); }
    // };

    // // Finally, try to convert into a semantic version number
    // let buildx_version = match Version::from_str(&buildx_version) {
    //     Ok(buildx_version) => buildx_version,
    //     Err(err)           => { return Err(UtilError::IllegalBuildxVersion{ version: buildx_version, err }); }
    // };

    // // With that all done, compare it with the required
    // if buildx_version < MIN_BUILDX_VERSION {
    //     return Ok(Err(DependencyError::BuildKitMinNotMet{ got: docker_version, expected: MIN_BUILDX_VERSION }));
    // }



    // We checked all the runtime dependencies!
    Ok(Ok(()))
}



/// **Edited: now returning CliErrors.**
/// 
/// Tries to determine the package file in the pulled repository.
/// 
/// **Arguments**
///  * `dir`: The directory the is the root of a package.
/// 
/// **Returns**  
/// A PathBuf pointing to what we think is the package file, or else a CliError if we could not determine it or something went wrong.
pub fn determine_file(
    dir: &Path,
) -> Result<PathBuf, UtilError> {
    // Open an iterator over the directory's files
    let files = match fs::read_dir(dir) {
        Ok(files) => files,
        Err(err)  => { return Err(UtilError::DirectoryReadError{ dir: dir.to_path_buf(), err }); }
    };

    // Iterate through them
    for file in files {
        // Make sure this file is valid
        let file = match file {
            Ok(file) => file,
            Err(err) => { return Err(UtilError::DirectoryReadError{ dir: dir.to_path_buf(), err }); }
        };

        // Compare the filename with anything we know
        let file_name = String::from(file.file_name().to_string_lossy()).to_lowercase();
        if file.path().is_file() &&
            (file_name.eq("container.yml") ||
             file_name.eq("container.yaml") ||
             file_name.ends_with(".bk") ||
             file_name.ends_with(".cwl"))
        {
            return Ok(PathBuf::from(file_name));
        }
    }

    Err(UtilError::UndeterminedPackageFile{ dir: dir.to_path_buf() })
}



/// **Edited: not taking a context anymore, returning CliErrors and a PackageKind instead of a string.**
/// 
/// Tries to deduce the package kind from the given file.
/// 
/// **Arguments**
///  * `path`: Path to file from which we'd like to deduce the kind.
/// 
/// **Returns**  
/// The PackageKind if we could deduce it, or some sort of CliError if we could not or something went wrong.
pub fn determine_kind(
    path: &Path,
) -> Result<PackageKind, UtilError> {
    // See if the filename convention allows us to choose a package kind
    if let Some(file) = path.file_name() {
        let filename = String::from(file.to_string_lossy()).to_lowercase();
        if filename.eq("container.yml") || filename.eq("container.yaml") {
            // It's a code package, likely
            return Ok(PackageKind::Ecu);
        }
    }
    // See if the extension allows us to choose a package kind
    if let Some(extension) = path.extension() {
        let extension = String::from(extension.to_string_lossy()).to_lowercase();
        if extension.eq("bk") {
            // It's a Bakery / DSL package
            return Ok(PackageKind::Dsl);
        }
    }

    // For CWL and OAS we need to look inside the file
    let mut file_content = String::new();
    {
        // Open the file
        let mut handle = match File::open(path) {
            Ok(handle) => handle,
            Err(err)   => { return Err(UtilError::PackageFileOpenError{ file: path.to_path_buf(), err }); }
        };

        // Read the entire file to the string
        if let Err(err) = handle.read_to_string(&mut file_content) {
            return Err(UtilError::PackageFileReadError{ file: path.to_path_buf(), err });
        };
    }

    // Check if the content contains a keywords that allow us to say which package it is
    if file_content.contains("cwlVersion") {
        return Ok(PackageKind::Cwl);
    }
    if file_content.contains("openapi") {
        return Ok(PackageKind::Oas);
    }

    // Could not determine the package
    Err(UtilError::UndeterminedPackageKind{ file: path.to_path_buf() })
}



/// **Edited: uses dirs_2 instead of appdirs and returns UtilErrors when it goes wrong.**
///
/// Returns the path of the configuration directory. Is guaranteed to be an absolute path when it returns successfully (but _not_ that it also exists!).
/// 
/// **Returns**  
/// The path of the Brane configuration directory if successful, or a UtilError otherwise.
pub fn get_config_dir() -> Result<PathBuf, UtilError> {
    // Try to get the user directory
    let user = match dirs_2::config_dir() {
        Some(user) => user,
        None       => { return Err(UtilError::UserConfigDirNotFound); }
    };

    // Simply append Brane's path and return
   Ok(user.join("brane"))
}

/// Makes sure that Brane's config directory exists and then returns its path.
/// 
/// **Arguments**
///  * `create`: If true, creates the directory if it does not exist; if false, throws an error.
/// 
/// **Returns**  
/// The path of the Brane configuration directory if successful, or a UtilError otherwise.
pub fn ensure_config_dir(create: bool) -> Result<PathBuf, UtilError> {
    // Get the brane directory
    let config_dir: PathBuf = get_config_dir()?;

    // Check if the brane directory exists
    if !config_dir.exists() {
        // Either create it if told to do so, or error
        if create { if let Err(err) = fs::create_dir_all(&config_dir) { return Err(UtilError::BraneConfigDirCreateError{ path: config_dir, err }); } }
        else { return Err(UtilError::BraneConfigDirNotFound{ path: config_dir }); }
    }

    // Done, return the path
    Ok(config_dir)
}

/// **Edited: Now returns UtilErrors.**
///
/// Returns the location of the history file for Brane.
/// 
/// **Returns**  
/// The path of the HistoryFile or a UtilError otherwise.
pub fn get_history_file() -> Result<PathBuf, UtilError> {
    // Get the config dir
    let config_dir = get_config_dir()?;

    // Add the path and return
    Ok(config_dir.join("repl_history.txt"))
}

/// Makes sure that the history file exists and then returns its path.
/// 
/// **Arguments**
///  * `create`: If true, creates the directory if it does not exist; if false, throws an error.
/// 
/// **Returns**  
/// The path of the HistoryFile or a UtilError otherwise.
pub fn ensure_history_file(create: bool) -> Result<PathBuf, UtilError> {
    // Get the path to the history file
    let history_file = get_history_file()?;

    // Make sure it exists
    if !history_file.exists() {
        // Either create it if told to do so, or error
        if create {
            // Make sure the config directory exists
            ensure_config_dir(create)?;

            // Now create the file
            if let Err(err) = File::create(&history_file) { return Err(UtilError::HistoryFileCreateError{ path: history_file, err }); }
        } else {
            return Err(UtilError::HistoryFileNotFound{ path: history_file });
        }
    }

    // Done
    Ok(history_file)
}



/// Returns the general data directory based on the user's home folder.
/// 
/// **Arguments**
///  * `create`: If set to true, creates the missing file and directories instead of throwing errors.
/// 
/// **Returns**  
/// A PathBuf with the absolute path that is guaranteed to exist, or an UtilError otherwise.
pub fn get_data_dir() -> Result<PathBuf, UtilError> {
    // Try to get the user directory
    let user = match dirs_2::data_local_dir() {
        Some(user) => user,
        None       => { return Err(UtilError::UserLocalDataDirNotFound); }
    };

    // Join the Brane directory and done
    Ok(user.join("brane"))
}

/// Makes sure that Brane's data directory exists, and then returns its path.
/// 
/// **Arguments**
///  * `create`: If true, creates the directory if it does not exist; if false, throws an error.
/// 
/// **Returns**  
/// A PathBuf with the absolute path that is guaranteed to exist, or an UtilError otherwise.
pub fn ensure_data_dir(create: bool) -> Result<PathBuf, UtilError> {
    // Get the data directory
    let data_dir = get_data_dir()?;

    // Check if the brane directory exists
    if !data_dir.exists() {
        // Either create it if told to do so, or error
        if create { if let Err(err) = fs::create_dir_all(&data_dir) { return Err(UtilError::BraneDataDirCreateError{ path: data_dir, err }); } }
        else { return Err(UtilError::BraneDataDirNotFound{ path: data_dir }); }
    }

    // Done (get_data_dir() is already absolute)
    Ok(data_dir)
}

/// **Edited: Changed to return UtilErrors.**
///
/// Returns the general package directory based on the user's home folder.  
/// Basically, tries to resolve the folder '~/.local/share/brane/packages`.  
/// Note that this does not mean that this directory exists.
/// 
/// **Returns**  
/// A PathBuf with an absolute path to the packages dir, or an UtilError otherwise.
pub fn get_packages_dir() -> Result<PathBuf, UtilError> {
    // Get the data directory
    let data_dir = get_data_dir()?;

    // Append the packages directory and done
    Ok(data_dir.join("packages"))
}

/// Makes sure that Brane's packages directory exists, and then returns its path.  
/// Basically, tries to resolve the folder '~/.local/share/brane/packages`.
/// 
/// **Arguments**
///  * `create`: If set to true, creates the missing file and directories instead of throwing errors.
/// 
/// **Returns**  
/// A PathBuf with the absolute path that is guaranteed to exist, or an UtilError otherwise.
pub fn ensure_packages_dir(create: bool) -> Result<PathBuf, UtilError> {
    // Get the packages directory
    let packages_dir = get_packages_dir()?;

    // Make sure it exists
    if !packages_dir.exists() {
        // Either create it if told to do so, or error
        if create {
            // Make sure the data directory exists
            ensure_data_dir(create)?;

            // Now create the directory
            if let Err(err) = fs::create_dir(&packages_dir) { return Err(UtilError::BranePackageDirCreateError{ path: packages_dir, err }); }
        } else {
            return Err(UtilError::BranePackageDirNotFound{ path: packages_dir });
        }
    }

    // Done, since the packages directory is always canonicalized
    Ok(packages_dir)
}

/// **Edited: Now returning UtilErrors.**
///
/// Gets the directory where we likely stored the package.  
/// If the given version is omitted, just returns the package directory for this name.  
/// If the given version is latest, tries to find the latest version directory to return that; otherwise, errors that there are no versions to choose from.  
/// Does not guarantee that the directory also exists; check ensure_package_dir() for that.
/// 
/// **Arguments**
///  * `name`: The name of the package we want to get the directory from.
///  * `version`: The version of the package. Is optional to have a package directory that ignores versions.
/// 
/// **Returns**  
/// A PathBuf with the directory if successfull, or an UtilError otherwise.
pub fn get_package_dir(
    name: &str,
    version: Option<&Version>,
) -> Result<PathBuf, UtilError> {
    // Try to get the general package directory + the name of the package
    let packages_dir = get_packages_dir()?;
    let package_dir = packages_dir.join(&name);

    // If there is no version, call it quits here
    if version.is_none() { return Ok(package_dir); }

    // Otherwise, resolve the version number if its 'latest'
    let version = version.unwrap();
    let version = if version.is_latest() {
        // Get the list of versions
        let mut versions = get_package_versions(name, &package_dir)?;

        // Sort the versions and return the last one
        versions.sort();
        versions[versions.len() - 1].clone()
    } else {
        // Simply use the given version
        version.clone()
    };

    // Return the path with the version appended to it
    Ok(package_dir.join(&version.to_string()))
}

/// Makes sure that the package directory for the given name/version pair exists, then returns the path to it.  
/// If the given version is omitted, just returns the package directory for this name.  
/// If the given version is latest, tries to find the latest version directory to return that; otherwise, always errors (regardless of 'create').
/// 
/// **Arguments**
///  * `name`: The name of the package we want to get the directory from.
///  * `version`: The version of the package. Is optional to have a package directory without any nested versions.
///  * `create`: If set to true, creates the missing file and directories instead of throwing errors.
/// 
/// **Returns**  
/// A PathBuf with the directory if successfull, or an UtilError otherwise.
pub fn ensure_package_dir(
    name: &str,
    version: Option<&Version>,
    create: bool,
) -> Result<PathBuf, UtilError> {
    // Retrieve the path for this version
    let package_dir = get_package_dir(name, version)?;

    // Make sure it exists
    if !package_dir.exists() {
        // Before we decide what to do, match on whether we have a version (to return more accurate errors)
        match version {
            Some(version) => {
                // Either create it if told to do so, or error
                if create {
                    // Make sure the packages directory exists
                    ensure_packages_dir(create)?;

                    // Now create the directory
                    if let Err(err) = fs::create_dir_all(&package_dir) { return Err(UtilError::VersionDirCreateError{ package: name.to_string(), version: version.clone(), path: package_dir, err }); }
                } else {
                    return Err(UtilError::VersionDirNotFound{ package: name.to_string(), version: version.clone(), path: package_dir });
                }
            },

            None => {
                // Either create it if told to do so, or error
                if create {
                    // Make sure the packages directory exists
                    ensure_packages_dir(create)?;

                    // Now create the directory
                    if let Err(err) = fs::create_dir_all(&package_dir) { return Err(UtilError::PackageDirCreateError{ package: name.to_string(), path: package_dir, err }); }
                } else {
                    return Err(UtilError::PackageDirNotFound{ package: name.to_string(), path: package_dir });
                }
            },
        }
    }

    // It's alright
    Ok(package_dir)
}

/// Collects a list of versions in the given package directory.
/// 
/// **Arguments**
///  * `package_name`: The name of the package we search the directory of (used for debugging purposes).
///  * `package_dir`: The package directory to search. This function assumes it already exists.
/// 
/// **Returns**  
/// The list of Versions found in the given package directory, or a PackageError if we couldn't.
pub fn get_package_versions(
    package_name: &str,
    package_dir: &Path,
) -> Result<Vec<Version>, UtilError> {
    // Get the list of available versions
    let version_dirs = match fs::read_dir(&package_dir) {
        Ok(files)   => files,
        Err(reason) => { return Err(UtilError::PackageDirReadError{ path: package_dir.to_path_buf(), err: reason }); }
    };

    // Convert the list of strings into a version
    let mut versions: Vec<Version> = Vec::new();
    for dir in version_dirs {
        if let Err(reason) = dir { return Err(UtilError::PackageDirReadError{ path: package_dir.to_path_buf(), err: reason }); }
        let dir_path = dir.unwrap().path();

        // Next, check if it's a 'package dir' by checking for the files we need
        if !dir_path.join("package.yml").exists() || dir_path.join(".lock").exists() {
            // It's not a version folder
            continue;
        }

        // Try to parse the filename as a version number
        let dir_name = match dir_path.file_name() {
            Some(value) => value.to_string_lossy().to_string(),
            None       => { return Err(UtilError::UnreadableVersionEntry{ path: dir_path }); }
        };
        let version = match Version::from_str(&dir_name) {
            Ok(value)   => value,
            Err(reason) => { return Err(UtilError::IllegalVersionEntry{ package: package_name.to_string(), version: dir_name, err: reason }); }
        };

        // Push it to the list and try again
        versions.push(version);
    }
    if versions.is_empty() { return Err(UtilError::NoVersions{ package: package_name.to_string() }); }

    // Done! Return it
    Ok(versions)
}



/// Returns an equivalent string to the given one, except that the first letter is capitalized.
/// 
/// **Arguments**
///  * `s`: The string to capitalize.
/// 
/// **Returns**  
/// A copy of the given string with the first letter in uppercase.
pub fn uppercase_first_letter(
    s: &str,
) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().chain(c).collect(),
    }
}



/// Checks whether the given string is a valid name for Bakery.
/// 
/// **Arguments**
///  * `name`: The name to check.
/// 
/// **Returns**  
/// Nothing if the name is valid, or a UtilError otherwise.
pub fn assert_valid_bakery_name(
    name: &str,
) -> Result<(), UtilError> {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(UtilError::InvalidBakeryName{ name: name.to_string() })
    }
}
