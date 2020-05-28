use crate::packages;
use specifications::common::Function;
use specifications::container::ContainerInfo;
use specifications::package::PackageInfo;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

type FResult<T> = Result<T, failure::Error>;
type Map<T> = std::collections::HashMap<String, T>;

const INIT_URL: &str = "https://github.com/brane-ri/entrypoint/releases/download/v0.2.0/brane-init";

///
///
///
pub fn handle(
    context: PathBuf,
    file: PathBuf,
) -> FResult<()> {
    let container_info = ContainerInfo::from_path(context.join(file))?;
    let package_dir = packages::get_package_dir(&container_info.name, Some(&container_info.version))?;

    // Prepare package directory
    let dockerfile = generate_dockerfile(&container_info)?;
    let package_info = generate_package_info(&container_info)?;
    prepare_directory(&container_info, dockerfile, &package_info, &package_dir)?;

    // Build ECU image
    let tag = format!("{}:{}", container_info.name, container_info.version);
    build_ecu_image(&package_dir, tag)?;

    Ok(())
}

///
///
///
fn generate_package_info(container_info: &ContainerInfo) -> FResult<PackageInfo> {
    // Construct function descriptions
    let mut functions = Map::<Function>::new();
    for (action_name, action) in &container_info.actions {
        let arguments = action.input.clone();
        let pattern = action.pattern.clone();
        let return_type = action.output[0].data_type.to_string();

        let function = Function::new(arguments, pattern, return_type);
        functions.insert(action_name.clone(), function);
    }

    // Create and write a package.yml file.
    let package_info = PackageInfo::new(
        container_info.name.clone(),
        container_info.version.clone(),
        container_info.description.clone(),
        String::from("ecu"),
        Some(functions),
        None,
    );

    Ok(package_info)
}

///
///
///
fn generate_dockerfile(container_info: &ContainerInfo) -> FResult<String> {
    let mut contents = String::new();
    let base = container_info
        .base
        .clone()
        .unwrap_or_else(|| String::from("ubuntu:20.04"));

    // Add default heading
    writeln!(contents, "# Generated by Brane")?;
    writeln!(contents, "FROM {}", base)?;

    // Add environemt variables
    if let Some(environment) = &container_info.environment {
        for (key, value) in environment {
            writeln!(contents, "ENV {}={}", key, value)?;
        }
    }

    // Add Brane entrypoint
    writeln!(contents, "ADD {} init", INIT_URL)?;
    writeln!(contents, "RUN chmod +x init")?;
    writeln!(contents, "ENTRYPOINT [\"./init\"]")?;

    // Add dependencies
    if base.starts_with("alpine") {
        write!(contents, "RUN apk add --no-cache ")?;
    } else {
        write!(contents, "RUN apt-get update && apt-get install -y ")?;
    }
    if let Some(dependencies) = &container_info.dependencies {
        for dependency in dependencies {
            write!(contents, "{} ", dependency)?;
        }
    }
    writeln!(contents)?;

    // Copy files
    writeln!(contents, "COPY container.yml /container.yml")?;
    writeln!(contents, "ADD wd.tar.gz /opt")?;
    writeln!(contents, "WORKDIR /opt/wd")?;

    // Add installation
    if let Some(install) = &container_info.install {
        for line in install {
            writeln!(contents, "RUN {}", line)?;
        }
    }

    writeln!(contents, "WORKDIR /")?;

    Ok(contents)
}

///
///
///
fn prepare_directory(
    container_info: &ContainerInfo,
    dockerfile: String,
    package_info: &PackageInfo,
    package_dir: &PathBuf,
) -> FResult<()> {
    fs::create_dir_all(&package_dir)?;

    // Write container.yml to package directory.
    let mut buffer = File::create(&package_dir.join("container.yml"))?;
    write!(buffer, "{}", serde_yaml::to_string(&container_info)?)?;

    // Write Dockerfile to package directory
    let mut buffer = File::create(package_dir.join("Dockerfile"))?;
    write!(buffer, "{}", dockerfile)?;

    // Write Dockerfile to package directory
    let mut buffer = File::create(package_dir.join("package.yml"))?;
    write!(buffer, "{}", serde_yaml::to_string(&package_info)?)?;

    // Create the working directory and copy required files.
    let wd = package_dir.join("wd");
    if let Some(files) = &container_info.files {
        for file in files {
            let wd_path = wd.join(file);
            if let Some(parent) = wd_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(&parent)?;
                }
            }

            fs::copy(file, &wd_path)?;
        }
    }

    // Archive the working directory and remove the original.
    let output = Command::new("tar")
        .arg("-zcf")
        .arg("wd.tar.gz")
        .arg("wd")
        .current_dir(&package_dir)
        .output()
        .expect("Couldn't run 'tar' command.");

    ensure!(output.status.success(), "Failed to prepare workdir.");

    let output = Command::new("rm")
        .arg("-rf")
        .arg("wd")
        .current_dir(&package_dir)
        .output()
        .expect("Couldn't run 'rm' command.");

    ensure!(output.status.success(), "Failed to prepare workdir.");

    Ok(())
}

///
///
///
fn build_ecu_image(
    package_dir: &PathBuf,
    tag: String,
) -> FResult<()> {
    let output = Command::new("docker")
        .arg("buildx")
        .arg("build")
        .arg("--output")
        .arg("type=docker,dest=image.tar")
        .arg("--tag")
        .arg(tag)
        .arg(".")
        .current_dir(&package_dir)
        .status()
        .expect("Couldn't run 'docker' command.");

    ensure!(output.success(), "Failed to build ECU image.");

    Ok(())
}
