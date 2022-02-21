use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use std::{fmt::Write as FmtWrite, path::Path};

use console::style;
use fs_extra::dir::CopyOptions;
use path_clean::clean as clean_path;
use specifications::container::ContainerInfo;
use specifications::package::PackageInfo;

use crate::build_common::{BRANELET_URL, JUICE_URL, build_docker_image, clean_directory, lock_directory, unlock_directory};
use crate::errors::BuildError;
use crate::packages;


/***** BUILD FUNCTIONS *****/
/// **Edited: Now wrapping around build() to handle the lock file properly.
/// 
/// **Arguments**
///  * `context`: The directory to copy additional files (executable, working directory files) from.
///  * `file`: Path to the package's main file (a container file, in this case).
///  * `branelet_path`: Optional path to a custom branelet executable. If left empty, will pull the standard one from Github instead.
///  * `keep_files`: Determines whether or not to keep the build files after building.
/// 
/// **Returns**  
/// Nothing if the package is build successfully, but a BuildError otherwise.
pub async fn handle(
    context: PathBuf,
    file: PathBuf,
    branelet_path: Option<PathBuf>,
    keep_files: bool,
) -> Result<(), BuildError> {
    debug!("Building ecu package from container file '{}'...", file.display());
    debug!("Using {} as build context", context.display());

    // Read the package into a ContainerInfo.
    let handle = match File::open(&file) {
        Ok(handle) => handle,
        Err(err)   => { return Err(BuildError::ContainerInfoOpenError{ file, err }); }
    };
    let reader = BufReader::new(handle);
    let document = match ContainerInfo::from_reader(reader) {
        Ok(document) => document,
        Err(err)     => { return Err(BuildError::ContainerInfoParseError{ file, err }); }
    };

    // Prepare package directory
    let package_info = PackageInfo::from(&document);
    let package_dir = match packages::get_package_dir(&package_info.name, Some(&package_info.version), true) {
        Ok(package_dir) => package_dir,
        Err(err)        => { return Err(BuildError::PackageDirError{ err }); }
    };

    // Lock the directory, build, unlock the directory
    lock_directory(&package_dir)?;
    let res = build(document, context, package_info, &package_dir, branelet_path, keep_files).await;
    unlock_directory(&package_dir);

    // Return the result of the build process
    res
}



/// Actually builds a new Ecu package from the given file(s).
/// 
/// **Arguments**
///  * `document`: The ContainerInfo document describing the package.
///  * `context`: The directory to copy additional files (executable, working directory files) from.
///  * `package_dir`: The package directory to use as the build folder.
///  * `package_info`: The PackageInfo document also describing the package, but in a package-kind-oblivious way.
///  * `branelet_path`: Optional path to a custom branelet executable. If left empty, will pull the standard one from Github instead.
///  * `keep_files`: Determines whether or not to keep the build files after building.
/// 
/// **Returns**  
/// Nothing if the package is build successfully, but a BuildError otherwise.
async fn build(
    document: ContainerInfo,
    context: PathBuf,
    package_info: PackageInfo,
    package_dir: &Path,
    branelet_path: Option<PathBuf>,
    keep_files: bool,
) -> Result<(), BuildError> {
    // Prepare the build directory
    let dockerfile = generate_dockerfile(&document, &context, branelet_path.is_some())?;
    prepare_directory(
        &document,
        dockerfile,
        branelet_path,
        &context,
        &package_info,
        &package_dir,
    )?;
    debug!("Successfully prepared package directory.");

    // Build Docker image
    let tag = format!("{}:{}", package_info.name, package_info.version);
    match build_docker_image(&package_dir, tag) {
        Ok(_) => {
            println!(
                "Successfully built version {} of container (ECU) package {}.",
                style(&package_info.version).bold().cyan(),
                style(&package_info.name).bold().cyan(),
            );
    
            // // Check if previous build is still loaded in Docker
            // let image_name = format!("{}:{}", package_info.name, package_info.version);
            // if let Err(e) = docker::remove_image(&image_name).await { return Err(BuildError::DockerCleanupError{ image: image_name, err }); }
    
            // // Upload the 
            // let image_name = format!("localhost:5000/library/{}", image_name);
            // if let Err(e) = docker::remove_image(&image_name).await { return Err(BuildError::DockerCleanupError{ image: image_name, err }); }
    
            // Remove all non-essential files.
            if !keep_files { clean_directory(&package_dir, vec![ "container.yml", "Dockerfile", "wd.tar.gz" ]); }
        },

        Err(err) => {
            // Print the error first
            eprintln!("{}", err);

            // Print some output message, and then cleanup
            println!(
                "Failed to built version {} of container (ECU) package {}. See error output above.",
                style(&package_info.version).bold().cyan(),
                style(&package_info.name).bold().cyan(),
            );
            if let Err(err) = fs::remove_dir_all(&package_dir) { return Err(BuildError::CleanupError{ path: package_dir.to_path_buf(), err }); }
        }
    }

    // Done
    Ok(())
}

/// **Edited: now returning BuildErrors.**
/// 
/// Generates a new DockerFile that can be used to build the package into a Docker container.
/// 
/// **Arguments**
///  * `document`: The ContainerInfo describing the package to build.
///  * `context`: The directory to find the executable in.
///  * `override_branelet`: Whether or not to override the branelet executable. If so, assumes the new one is copied to the temporary build folder by the time the DockerFile is run.
/// 
/// **Returns**  
/// A String that is the new DockerFile on success, or a BuildError otherwise.
fn generate_dockerfile(
    document: &ContainerInfo,
    context: &Path,
    override_branelet: bool,
) -> Result<String, BuildError> {
    let mut contents = String::new();

    // Get the base image from the document
    let base = document.base.clone().unwrap_or(String::from("ubuntu:20.04"));

    // Add default heading
    writeln_build!(contents, "# Generated by Brane")?;
    writeln_build!(contents, "FROM {}", base)?;

    // Add environemt variables
    if let Some(environment) = &document.environment {
        for (key, value) in environment {
            writeln_build!(contents, "ENV {}={}", key, value)?;
        }
    }

    // Add dependencies; write the apt-get RUN command with space for packages
    if base.starts_with("alpine") {
        write_build!(contents, "RUN apk add --no-cache ")?;
    } else {
        write_build!(contents, "RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --allow-change-held-packages --allow-downgrades ")?;
    }
    // Default dependencies
    write_build!(contents, "fuse iptables ")?;
    // Custom dependencies
    if let Some(dependencies) = &document.dependencies {
        for dependency in dependencies {
            write_build!(contents, "{} ", dependency)?;
        }
    }
    writeln_build!(contents)?;

    // Add the branelet executable
    if override_branelet {
        // It's the custom in the temp dir
        writeln_build!(contents, "ADD branelet branelet")?;
    } else {
        // It's the prebuild one
        writeln_build!(contents, "ADD {} branelet", BRANELET_URL)?;
    }
    // Always make it executable
    writeln_build!(contents, "RUN chmod +x branelet")?;

    // Add JuiceFS
    writeln_build!(contents, "ADD {} juicefs.tar.gz", JUICE_URL)?;
    writeln_build!(
        contents,
        "RUN tar -xzf juicefs.tar.gz && rm juicefs.tar.gz && mkdir /data"
    )?;

    // Copy the package files
    writeln_build!(contents, "COPY container.yml /container.yml")?;
    writeln_build!(contents, "ADD wd.tar.gz /opt")?;
    writeln_build!(contents, "WORKDIR /opt/wd")?;

    // Copy the entrypoint executable
    let entrypoint = clean_path(&document.entrypoint.exec);
    if entrypoint.contains("..") { return Err(BuildError::UnsafePath{ path: entrypoint }); }
    let entrypoint = context.join(entrypoint);
    if !entrypoint.exists() || !entrypoint.is_file() { return Err(BuildError::MissingExecutable{ path: entrypoint }); }
    writeln_build!(contents, "RUN chmod +x /opt/wd/{}", &document.entrypoint.exec)?;

    // Add installation script
    if let Some(install) = &document.install {
        for line in install {
            writeln_build!(contents, "RUN {}", line)?;
        }
    }

    // Finally, add branelet as the entrypoint
    writeln_build!(contents, "ENTRYPOINT [\"/branelet\"]")?;

    // Done!
    Ok(contents)
}

/// **Edited: now returning BuildErrors.**
/// 
/// Prepares the build directory for building the package.
/// 
/// **Arguments**
///  * `document`: The ContainerInfo document carrying metadata about the package.
///  * `dockerfile`: The generated DockerFile that will be used to build the package.
///  * `branelet_path`: The optional branelet path in case we want it overriden.
///  * `context`: The directory to copy additional files (executable, working directory files) from.
///  * `package_info`: The generated PackageInfo from the ContainerInfo document.
///  * `package_dir`: The directory where we can build the package and store it once done.
/// 
/// **Returns**  
/// Nothing if the directory was created successfully, or a BuildError otherwise.
fn prepare_directory(
    document: &ContainerInfo,
    dockerfile: String,
    branelet_path: Option<PathBuf>,
    context: &Path,
    package_info: &PackageInfo,
    package_dir: &Path,
) -> Result<(), BuildError> {
    // Write container.yml to package directory.
    let container_path = package_dir.join("container.yml");
    match File::create(&container_path) {
        Ok(ref mut handle) => {
            // Try to serialize the document
            let to_write = match serde_yaml::to_string(&document) {
                Ok(to_write) => to_write,
                Err(err)     => { return Err(BuildError::ContainerInfoSerializeError{ err }); }
            };
            if let Err(err) = write!(handle, "{}", to_write) {
                return Err(BuildError::PackageFileWriteError{ path: container_path, err });
            }
        },
        Err(err)   => { return Err(BuildError::PackageFileCreateError{ path: container_path, err }); }
    };

    // Write Dockerfile to package directory
    let file_path = package_dir.join("Dockerfile");
    match File::create(&file_path) {
        Ok(ref mut handle) => {
            if let Err(err) = write!(handle, "{}", dockerfile) {
                return Err(BuildError::PackageFileWriteError{ path: file_path, err });
            }
        },
        Err(err)   => { return Err(BuildError::PackageFileCreateError{ path: file_path, err }); }
    };

    // Write package.yml to package directory
    let package_path = package_dir.join("package.yml");
    match File::create(&package_path) {
        Ok(ref mut handle) => {
            // Try to serialize the document
            let to_write = match serde_yaml::to_string(&package_info) {
                Ok(to_write) => to_write,
                Err(err)     => { return Err(BuildError::PackageInfoSerializeError{ err }); }
            };
            if let Err(err) = write!(handle, "{}", to_write) {
                return Err(BuildError::PackageFileWriteError{ path: package_path, err });
            }
        },
        Err(err)   => { return Err(BuildError::PackageFileCreateError{ path: package_path, err }); }
    };

    // Copy custom branelet binary to package directory if needed
    if let Some(branelet_path) = branelet_path {
        // Try to resole the branelet's path
        let source = match std::fs::canonicalize(&branelet_path) {
            Ok(source) => source,
            Err(err)   => { return Err(BuildError::BraneletCanonicalizeError{ path: branelet_path, err }); }
        };
        let target = package_dir.join("branelet");
        if let Err(err) = fs::copy(&source, &target) {
            return Err(BuildError::BraneletCopyError{ source, target, err });
        }
    }

    // Create a workdirectory and make sure it's empty
    let wd = package_dir.join("wd");
    if wd.exists() {
        if let Err(err) = fs::remove_dir_all(&wd) {
            return Err(BuildError::WdClearError{ path: wd, err });
        } 
    }
    if let Err(err) = fs::create_dir(&wd) {
        return Err(BuildError::WdCreateError{ path: wd, err });
    }

    // Always copy these two files, required by convention
    let target = wd.join("container.yml");
    if let Err(err) = fs::copy(&container_path, &target) { return Err(BuildError::WdFileCopyError{ source: container_path, target, err }); };
    let target = wd.join("package.yml");
    if let Err(err) = fs::copy(&package_path, &target)   { return Err(BuildError::WdFileCopyError{ source: package_path, target, err }); };

    // Copy any other files marked in the ecu document
    if let Some(files) = &document.files {
        for file_path in files {
            // Make sure the target path is safe (does not escape the working directory)
            let target = clean_path(&file_path);
            if target.contains("..") { return Err(BuildError::UnsafePath{ path: target }) }
            let target = wd.join(target);
            let target = match fs::canonicalize(target.parent().expect(&format!("Target file '{}' for package info file does not have a parent; this should never happen!", target.display()))) {
                Ok(target_dir) => target_dir.join(target.file_name().expect(&format!("Target file '{}' for package info file does not have a file name; this should never happen!", target.display()))),
                Err(err)       => { return Err(BuildError::WdSourceFileCanonicalizeError{ path: target, err }); }
            };
            // Create the target folder if it does not exist
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    if let Err(err) = fs::create_dir_all(parent) { return Err(BuildError::WdDirCreateError{ path: parent.to_path_buf(), err }); };
                }
            }

            // Resolve the source folder
            let source = match fs::canonicalize(context.join(file_path)) {
                Ok(source) => source,
                Err(err)   => { return Err(BuildError::WdTargetFileCanonicalizeError{ path: target, err }); }
            };

            // Switch whether it's a directory or a file
            if source.is_dir() {
                // Copy everything inside the folder
                let mut copy_options = CopyOptions::new();
                copy_options.copy_inside = true;
                if let Err(err) = fs_extra::dir::copy(&source, &target, &copy_options) { return Err(BuildError::WdDirCopyError{ source, target, err }); }
            } else {
                // Copy only the file
                if let Err(err) = fs::copy(&source, &target) { return Err(BuildError::WdFileCopyError{ source, target, err }); }
            }

            // Done
            debug!("Copied {} to {} in the working directory", source.display(), target.display());
        }
    }

    // Archive the working directory and remove the original.
    let mut command = Command::new("tar");
    command.arg("-zcf");
    command.arg("wd.tar.gz");
    command.arg("wd");
    command.current_dir(&package_dir);
    let output = match command.output() {
        Ok(output) => output,
        Err(err)   => { return Err(BuildError::WdCompressionLaunchError{ command: format!("{:?}", command), err }); }
    };
    if !output.status.success() {
        return Err(BuildError::WdCompressionError{ command: format!("{:?}", command), code: output.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&output.stdout).to_string(), stderr: String::from_utf8_lossy(&output.stderr).to_string() });
    }

    // Remove the working directory itself now we've compressed
    let mut command = Command::new("rm");
    command.arg("-rf");
    command.arg("wd");
    command.current_dir(&package_dir);
    let output = match command.output() {
        Ok(output) => output,
        Err(err)   => { return Err(BuildError::WdRemoveLaunchError{ command: format!("{:?}", command), err }); }
    };
    if !output.status.success() {
        return Err(BuildError::WdRemoveError{ command: format!("{:?}", command), code: output.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&output.stdout).to_string(), stderr: String::from_utf8_lossy(&output.stderr).to_string() });
    }

    // We're done with the working directory zip!
    Ok(())
}
