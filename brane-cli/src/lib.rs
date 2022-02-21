#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;
#[macro_use]
extern crate prettytable;
#[macro_use]
extern crate lazy_static;

#[macro_use]
pub mod build_common;
pub mod build_ecu;
pub mod build_oas;
pub mod docker;
pub mod errors;
pub mod packages;
pub mod registry;
pub mod repl;
pub mod run;
pub mod test;
pub mod utils;

use anyhow::Result;
use errors::CliError;
use semver::Version;
use specifications::package::PackageKind;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::{
    fs::{self, File},
    path::Path,
};

const MIN_DOCKER_VERSION: &str = "19.0.0";

///
///
///
pub fn check_dependencies() -> Result<()> {
    let output = Command::new("docker").arg("--version").output()?;
    let version = String::from_utf8_lossy(&output.stdout[15..17]);

    let version = Version::parse(&format!("{}.0.0", version))?;
    let minimum = Version::parse(MIN_DOCKER_VERSION)?;

    if version < minimum {
        return Err(anyhow!("Installed Docker doesn't meet the minimum requirement."));
    }

    Ok(())
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
pub fn determine_file(dir: &Path) -> Result<PathBuf, CliError> {
    // Open an iterator over the directory's files
    let files = match fs::read_dir(dir) {
        Ok(files) => files,
        Err(err)  => { return Err(CliError::DirectoryReadError{ dir: dir.to_path_buf(), err }); }
    };

    // Iterate through them
    for file in files {
        // Make sure this file is valid
        let file = match file {
            Ok(file) => file,
            Err(err) => { return Err(CliError::DirectoryReadError{ dir: dir.to_path_buf(), err }); }
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

    Err(CliError::UndeterminedPackageFile{ dir: dir.to_path_buf() })
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
) -> Result<PackageKind, CliError> {
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
            Err(err)   => { return Err(CliError::PackageFileOpenError{ file: path.to_path_buf(), err }); }
        };

        // Read the entire file to the string
        if let Err(err) = handle.read_to_string(&mut file_content) {
            return Err(CliError::PackageFileReadError{ file: path.to_path_buf(), err });
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
    Err(CliError::UndeterminedPackageKind{ file: path.to_path_buf() })
}
