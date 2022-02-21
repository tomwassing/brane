use crate::docker;
use anyhow::Result;
use bollard::errors::Error;
use bollard::image::ImportImageOptions;
use bollard::image::TagImageOptions;
use bollard::models::BuildInfo;
use bollard::Docker;
use chrono::Utc;
use console::{pad_str, Alignment};
use dialoguer::Confirm;
use fs_extra::dir;
use futures_util::stream::TryStreamExt;
use hyper::Body;
use indicatif::{DecimalBytes, HumanDuration};
use prettytable::format::FormatBuilder;
use prettytable::Table;
use semver::Version;
use serde_json::json;
use specifications::errors::SystemDirectoryError;
use specifications::package::{PackageIndex, PackageInfo, PackageInfoError, PackageIndexError};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs::File as TFile;
use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, FramedRead};


/* TIM */
/***** ERRORS *****/
/// Lists the errors that can occur when trying to do stuff with packages
#[derive(Debug)]
pub enum PackageError {
    /// Could not find a system directory
    SystemDirectoryError{ err: SystemDirectoryError },
    /// The Brane data/package directory doesn't exist
    BranePackageDirNotFound{ path: PathBuf },
    /// The Brane package directory could not be canonicalized
    BranePackageDirCanonicalizeError{ path: PathBuf, err: std::io::Error },
    /// A package directory does not exist
    PackageDirNotFound{ package: String, path: PathBuf },
    /// A package directory does not exist / could not be resolved
    PackageDirCanonicalizeError{ package: String, path: PathBuf, err: std::io::Error },

    /// We found a non-directory entry in the packages directory
    NoDirPackageEntry{ path: PathBuf },
    /// We found a package directory for a package but no versions in it
    NoVersions{ package: String },
    /// We found a non-directory entry in a package directory
    NoDirVersionEntry{ package: String, path: PathBuf },
    /// We couldn't get the filename component properly from a directory in the package directory
    UnreadableVersionEntry{ package: String, path: PathBuf },
    /// We found a non-directory or a non-version-number in a package directory
    IllegalVersionEntry{ package: String, path: PathBuf, err: semver::Error },
    /// We tried to load a package YML but failed
    InvalidPackageYml{ package: String, path: PathBuf, err: PackageInfoError },
    /// We tried to load a Package Index from a JSON value with PackageInfos but we failed
    PackageIndexError{ err: PackageIndexError },

    /// There was an error reading from files or directories
    ReadIOError{ path: PathBuf, err: std::io::Error },
    /// There was an error writing to files or directories
    WriteIOError{ path: PathBuf, err: std::io::Error },
}

impl PackageError {
    /// Static helper function that tries to resolve the type of a given path.
    /// 
    /// **Arguments**
    ///  * `path`: The PathBuf to resolve its type of.
    /// 
    /// **Returns**  
    /// " file" if the path points to a file; " directory" if it points to a directory; or "" if it's none of those.
    fn get_pathtype(path: &PathBuf) -> &str {
        if path.is_file() { " file" }
        else if path.is_dir() { " dir" }
        else { "" }
    }
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::SystemDirectoryError{ err }                       => write!(f, "{}", err),
            PackageError::BranePackageDirNotFound{ path }                   => write!(f, "Brane package directory '{}' not found", path.display()),
            PackageError::BranePackageDirCanonicalizeError{ path, err }     => write!(f, "Could not resolve Brane package directory '{}': {}", path.display(), err),
            PackageError::PackageDirNotFound{ package, path }               => write!(f, "Package directory '{}' for package '{}' not found", path.display(), package),
            PackageError::PackageDirCanonicalizeError{ package, path, err } => write!(f, "Could not resolve package directory '{}' for package '{}': {}", path.display(), package, err),

            PackageError::NoDirPackageEntry{ path }                 => write!(f, "Found non-directory package entry '{}' in directory of packages", path.display()),
            PackageError::NoVersions{ package }                     => write!(f, "Found directory for package '{}', but found no registered versions for it", package),
            PackageError::NoDirVersionEntry{ package, path }        => write!(f, "Found a non-directory entry '{}' for package '{}'", path.display(), package),
            PackageError::UnreadableVersionEntry{ package, path }   => write!(f, "Cannot determine name of entry '{}' for package '{}'", path.display(), package),
            PackageError::IllegalVersionEntry{ package, path, err } => write!(f, "Found version entry '{}' for package '{}' which cannot be converted to a version number: {}", path.file_name().unwrap().to_string_lossy(), package, err),
            PackageError::InvalidPackageYml{ package, path, err }   => write!(f, "Could not read '{}' for package '{}': {}", path.display(), package, err),
            PackageError::PackageIndexError{ err }                  => write!(f, "Could not create PackageIndex: {}", err),

            PackageError::ReadIOError{ path, err }  => write!(f, "Failed to read from{} '{}': {}", Self::get_pathtype(&path), path.display(), err),
            PackageError::WriteIOError{ path, err } => write!(f, "Failed to write to{} '{}': {}", Self::get_pathtype(&path), path.display(), err),
        }
    }
}

impl std::error::Error for PackageError {}





/***** HELPER FUNCTIONS *****/
/// Collects a list of versions in the given package directory.
/// 
/// **Arguments**
///  * `package_name`: The name of the package we search the directory of (used for debugging purposes).
///  * `package_dir`: The package directory to search. This function assumes it already exists.
/// 
/// **Returns**  
/// The list of Versions found in the given package directory, or a PackageError if we couldn't.
pub fn get_package_versions(package_name: &str, package_dir: &PathBuf) -> Result<Vec<Version>, PackageError> {
    // Get the list of available versions
    let version_dirs = match fs::read_dir(&package_dir) {
        Ok(files)   => files,
        Err(reason) => { return Err(PackageError::ReadIOError{ path: package_dir.clone(), err: reason }); }
    };

    // Convert the list of strings into a version
    let mut versions: Vec<Version> = Vec::new();
    for dir in version_dirs {
        if let Err(reason) = dir { return Err(PackageError::ReadIOError{ path: package_dir.clone(), err: reason }); }
        let dir = dir.unwrap();

        // First, make sure the dir points to a directory
        let dir_path = dir.path();
        if !dir_path.is_dir() { return Err(PackageError::NoDirVersionEntry{ package: package_name.to_string(), path: dir_path }); }

        // Next, check if it's a 'package dir' by checking for the files we need
        if !dir_path.join("package.yml").exists() || dir_path.join(".lock").exists() {
            // It's not a version file
            continue;
        }

        // Try to parse the filename as a version number
        let dir_name = match dir_path.file_name() {
            Some(value) => match value.to_str() {
                Some(value) => value,
                None        => { return Err(PackageError::UnreadableVersionEntry{ package: package_name.to_string(), path: dir_path }); }
            },
            None       => { return Err(PackageError::UnreadableVersionEntry{ package: package_name.to_string(), path: dir_path }); }
        };
        let version = match Version::parse(dir_name) {
            Ok(value)   => value,
            Err(reason) => { return Err(PackageError::IllegalVersionEntry{ package: package_name.to_string(), path: dir_path, err: reason }); }
        };

        // Push it to the list and try again
        versions.push(version);
    }
    if versions.len() == 0 { return Err(PackageError::NoVersions{ package: package_name.to_string() }); }

    // Done! Return it
    Ok(versions)
}

/// Inserts a PackageInfo in a list of PackageInfos such that it tries to only have the latest version of each package.
/// 
/// **Arguments**
///  * `infos`: The list of PackageInfos to insert into.
///  * `name`: The name of the package to add.
///  * `info`: The PackageInfo of the package to add.
fn insert_package_in_list(infos: &mut Vec<PackageInfo>, info: PackageInfo) {
    // Go through the list
    for pkg in infos.iter_mut() {
        // Check if its this package
        debug!("Package '{}' vs '{}'", &info.name, &pkg.name);
        if info.name.eq(&pkg.name) {
            // Only add if the new version is higher
            debug!(" > Version '{}' vs '{}'", info.version.to_string(), pkg.version.to_string());
            if info.version > pkg.version {
                *pkg = info;
            }
            // Always stop tho
            return;
        }
    }

    // Simply add to the list
    infos.push(info);
}
/*******/





/* TIM */
/// **Edited: Changed to return PackageErrors.**
///
/// Returns the general package directory based on the user's home folder.  
/// Basically, tries to resolve the folder '~/.local/share/brane/packages`.
/// 
/// **Returns**  
/// A PathBuf with the resolves path that is guaranteed to exist, or a PackageError otherwise.
pub fn get_packages_dir() -> Result<PathBuf, PackageError> {
    // Try to get the user directory
    let user = match dirs_2::data_local_dir() {
        Some(user) => user,
        None       => { return Err(PackageError::SystemDirectoryError{ err: SystemDirectoryError::UserLocalDataDirNotFound }); }
    };

    // Check if the brane directory exists and return the path if it does
    let path = user.join("brane");
    if !path.exists() { return Err(PackageError::SystemDirectoryError{ err: SystemDirectoryError::BraneLocalDataDirNotFound{ path: path } }); }

    // Finally, append the 'packages' part
    let path = path.join("packages");
    if !path.exists() { return Err(PackageError::BranePackageDirNotFound{ path: path }); }

    // Finally, canonicalize the path and return
    match fs::canonicalize(&path) {
        Ok(path) => Ok(path),
        Err(err) => Err(PackageError::BranePackageDirCanonicalizeError{ path, err }),
    }
}
/*******/

/* TIM */
/// **Edited: Now returning PackageErrors and added the 'create' parameter.**
///
/// Gets the directory where we likely stored the package.
/// 
/// **Arguments**
///  * `name`: The name of the package we want to get the directory from.
///  * `version`: The version of the package, already encoded as a string (and to accomodate 'latest').
///  * `create`: If true, creates missing directories instead of throwing errors.
/// 
/// **Returns**  
/// A PathBuf with the directory if successfull, or a PackageError otherwise.
pub fn get_package_dir(
    name: &str,
    version: Option<&str>,
    create: bool,
) -> Result<PathBuf, PackageError> {
    // Try to get the general package directory
    let packages_dir = get_packages_dir()?;
    debug!("Using Brane packages directory: '{}'", packages_dir.display());

    // Add the package name to the general directory
    let package_dir = packages_dir.join(&name);

    // Create the directory if it doesn't exist (or error)
    if !package_dir.exists() {
        if create { if let Err(err) = fs::create_dir(&package_dir) { return Err(PackageError::WriteIOError{ path: package_dir, err }); } }
        else { return Err(PackageError::PackageDirNotFound{ package: name.to_string(), path: package_dir }); }
    }

    // If there's no version, we call it quits here
    if version.is_none() {
        return Ok(package_dir);
    }

    // Otherwise, resolve the version number if its 'latest'
    let version = version.unwrap();
    let version = if version == "latest" {
        // Get the list of versions
        let mut versions = get_package_versions(name, &package_dir)?;

        // Sort the versions and return the last one
        versions.sort();
        versions[versions.len() - 1].clone()
    } else {
        // Simply try to parse the semantic version
        match Version::parse(version) {
            Ok(value) => value,
            Err(reason) => { return Err(PackageError::IllegalVersionEntry{ package: name.to_string(), path: package_dir.join(version), err: reason }); }
        }
    };

    // Verify if the target path exists
    let package_dir = package_dir.join(version.to_string());
    if !package_dir.exists() {
        if create { if let Err(err) = fs::create_dir(&package_dir) { return Err(PackageError::WriteIOError{ path: package_dir, err }); } }
        else { return Err(PackageError::PackageDirNotFound{ package: name.to_string(), path: package_dir }); }
    }

    // It does! We made it!
    debug!("Using package directory: '{}'", package_dir.display());
    Ok(package_dir)
}
/*******/

/* TIM */
/// **Edited: Changed to return PackageErrors.**
///
/// Returns the an index of available packages and their versions.
/// 
/// **Returns**  
/// A PackageIndex if we could retrieve it, or a PackageError if we failed.
pub fn get_package_index() -> Result<PackageIndex, PackageError> {
    // Try to get the generic packages dir (which is guaranteed to exist)
    let packages_dir = get_packages_dir()?;

    // Open an iterator to the list of files
    let package_dirs = match fs::read_dir(&packages_dir) {
        Ok(dir)     => dir,
        Err(reason) => { return Err(PackageError::ReadIOError{ path: packages_dir, err: reason }); }
    };

    // Start iterating through all the packages
    let mut packages = vec![];
    for package in package_dirs {
        if let Err(reason) = package { return Err(PackageError::ReadIOError{ path: packages_dir, err: reason }); }
        let package = package.unwrap();

        // Make sure it's a directory
        let package_path = package.path();
        if !package_path.is_dir() { return Err(PackageError::NoDirPackageEntry{ path: package_path }); }

        // Read the versions inside the package directory and add each of them separately
        let package_name = package_path.file_name().unwrap().to_string_lossy();
        let versions = get_package_versions(&package_name, &package_path)?;
        for version in versions {
            // Get the path of this version
            let version_path = package_path.join(version.to_string());

            // Try to read the propery package info
            let package_file = version_path.join("package.yml");
            match PackageInfo::from_path(package_file.clone()) {
                Ok(package_info) => { packages.push(package_info); }
                Err(reason)      => { return Err(PackageError::InvalidPackageYml{ package: package_name.to_string(), path: package_file, err: reason }); }
            }
        }
    }

    // Generate the package index from the collected list of packages
    match PackageIndex::from_value(json!(packages)) {
        Ok(index)   => Ok(index),
        Err(reason) => Err(PackageError::PackageIndexError{ err: reason }),
    }
}
/*******/

///
///
///
pub fn inspect(
    name: String,
    version: String,
) -> Result<()> {
    let package_dir = get_package_dir(&name, Some(version).as_deref(), false)?;
    let package_file = package_dir.join("package.yml");

    if let Ok(package_info) = PackageInfo::from_path(package_file) {
        println!("{:#?}", package_info);
    } else {
        return Err(anyhow!("Failed to read package information."));
    }

    Ok(())
}

/* TIM */
/// **Edited: updated to deal with get_packages_dir() returning ExecutorErrors. Also added option to only show latest packages and also standard packages.**
///
/// Lists the packages locally build and available.
/// 
/// **Arguments**
///  * `all`: If set to true, also shows standard packages.
///  * `latest`: If set to true, only shows latest version of each package.
/// 
/// **Returns**  
/// Nothing other than prints on stdout if successfull, or an ExecutorError otherwise.
pub fn list(all: bool, latest: bool) -> Result<(), PackageError> {
    // Get the directory with the packages
    let packages_dir = match get_packages_dir() {
        Ok(dir)     => dir,
        Err(_)      => { println!("No packages found."); return Ok(()); }
    };

    // Prepare display table.
    let format = FormatBuilder::new()
        .column_separator('\0')
        .borders('\0')
        .padding(1, 1)
        .build();
    let mut table = Table::new();
    table.set_format(format);
    table.add_row(row!["ID", "NAME", "VERSION", "KIND", "CREATED", "SIZE"]);

    // Get the local PackageIndex
    let index = match get_package_index() {
        Ok(idx) => idx,
        Err(reason) => { return Err(reason); }
    };

    // Collect a list of PackageInfos to show
    let mut infos: Vec<PackageInfo> = Vec::with_capacity(index.packages.len());
    // Do the standard packages first if told to do so
    if all {
        for (_, info) in index.standard {
            // Decide if we want to show all or just the latest version
            if latest {
                // Insert using the common code
                insert_package_in_list(&mut infos, info);
            } else {
                // Just append
                infos.push(info);
            }
        }
    }
    // Then to the normal packages
    for (_, info) in index.packages {
        // Decide if we want to show all or just the latest version
        if latest {
            // Insert using the common code
            insert_package_in_list(&mut infos, info);
        } else {
            // Just append
            infos.push(info);
        }
    }

    // With the list constructed, add each entry
    let now = Utc::now().timestamp();
    for entry in infos {
        // Derive the pathname for this package
        let package_path = packages_dir.join(&entry.name).join(entry.version.to_string());

        // Collect the package information in the proper formats
        let uuid = format!("{}", &entry.id);
        let id = pad_str(&uuid[..8], 10, Alignment::Left, Some(".."));
        let name = pad_str(&entry.name, 20, Alignment::Left, Some(".."));
        let version = pad_str(&entry.version, 10, Alignment::Left, Some(".."));
        let skind = format!("{}", entry.kind);
        let kind = pad_str(&skind, 10, Alignment::Left, Some(".."));
        let elapsed = Duration::from_secs((now - entry.created.timestamp()) as u64);
        let created = format!("{} ago", HumanDuration(elapsed));
        let created = pad_str(&created, 15, Alignment::Left, None);
        let size = DecimalBytes(dir::get_size(package_path).unwrap());

        // Add the row
        table.add_row(row![id, name, version, kind, created, size]);
    }
    
    // Write to stdout and done!
    table.printstd();
    Ok(())
}
/*******/

///
///
///
pub async fn load(
    name: String,
    version: Option<String>,
) -> Result<()> {
    debug!("Loading package '{}' (version {})", name, if version.is_some() { version.clone().unwrap() } else { String::from("-") });

    let version_or_latest = version.unwrap_or_else(|| String::from("latest"));
    let package_dir = get_package_dir(&name, Some(&version_or_latest), false)?;
    if !package_dir.exists() {
        return Err(anyhow!("Package not found."));
    }

    let package_info = PackageInfo::from_path(package_dir.join("package.yml"))?;
    let image = format!("{}:{}", package_info.name, package_info.version);
    let image_file = package_dir.join("image.tar");

    let docker = Docker::connect_with_local_defaults()?;

    // Abort, if image is already loaded
    if docker.inspect_image(&image).await.is_ok() {
        println!("Image already exists in local Docker deamon.");
        return Ok(());
    }

    println!("Image doesn't exist in Docker deamon: importing...");
    let options = ImportImageOptions { quiet: true };

    /* TIM */
    let file_handle = TFile::open(&image_file).await;
    if let Err(reason) = file_handle {
        let code = reason.raw_os_error().unwrap_or(-1);
        eprintln!("Could not open image file '{}': {}.", image_file.to_string_lossy(), reason);
        std::process::exit(code);
    }
    // let file = TFile::open(image_file).await?;
    let file = file_handle.ok().unwrap();
    /*******/
    let byte_stream = FramedRead::new(file, BytesCodec::new()).map(|r| {
        let bytes = r.unwrap().freeze();
        Ok::<_, Error>(bytes)
    });

    let body = Body::wrap_stream(byte_stream);
    let result = docker.import_image(options, body, None).try_collect::<Vec<_>>().await?;
    if let Some(BuildInfo {
        stream: Some(stream), ..
    }) = result.first()
    {
        debug!("{}", stream);

        let (_, image_hash) = stream.trim().split_once("sha256:").unwrap_or_default();

        // Manually add tag to image, if not specified.
        if !image_hash.is_empty() {
            debug!("Imported image: {}", image_hash);

            let options = TagImageOptions {
                repo: &package_info.name,
                tag: &package_info.version,
            };

            docker.tag_image(image_hash, Some(options)).await?;
        }
    }

    Ok(())
}

///
///
///
pub async fn remove(
    name: String,
    version: Option<String>,
    force: bool,
) -> Result<()> {
    // Remove without confirmation if explicity stated package version.
    if let Some(version) = version {
        let package_dir = get_package_dir(&name, Some(&version), false)?;
        if fs::remove_dir_all(&package_dir).is_err() {
            println!("No package with name '{}' and version '{}' exists!", name, version);
        }

        return Ok(());
    }

    let package_dir = get_package_dir(&name, None, false)?;
    if !package_dir.exists() {
        println!("No package with name '{}' exists!", name);
        return Ok(());
    }

    // Look for packages.
    let versions = fs::read_dir(&package_dir)?
        .map(|v| v.unwrap().file_name())
        .map(|v| String::from(v.to_string_lossy()))
        .collect::<Vec<String>>();

    // Ask for permission, if --force is not provided
    if !force {
        println!("Do you want to remove the following version(s)?");
        for version in &versions {
            println!("- {}", version);
        }
        println!();

        // Abort, if not approved
        if !Confirm::new().interact()? {
            return Ok(());
        }
    }

    // Check if image is locally loaded in Docker
    for version in &versions {
        let image_name = format!("{}:{}", name, version);
        docker::remove_image(&image_name).await?;

        let image_name = format!("localhost:5000/library/{}:{}", name, version);
        docker::remove_image(&image_name).await?;
    }

    fs::remove_dir_all(&package_dir)?;

    Ok(())
}
