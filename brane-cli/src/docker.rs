use crate::utils::get_package_dir;
use anyhow::Result;
use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions, StartContainerOptions,
    WaitContainerOptions
};
use bollard::errors::Error;
use bollard::image::{CreateImageOptions, ImportImageOptions, RemoveImageOptions};
use bollard::models::{DeviceRequest, HostConfig};
use bollard::Docker;
use brane_bvm::executor::{VmExecutor, ExecutorError};
use futures_util::stream::TryStreamExt;
use futures_util::StreamExt;
use hyper::Body;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use specifications::common::{FunctionExt, Value};
use specifications::errors::EncodeDecodeError;
use specifications::package::PackageInfo;
use std::env;
use std::path::PathBuf;
use std::{collections::HashMap, default::Default, path::Path};
use tokio::fs::File as TFile;
use tokio_util::codec::{BytesCodec, FramedRead};
use uuid::Uuid;

/// The standard return code which we accept as good status
const OK_RETURN_CODE: i32 = 0; 

lazy_static! {
    static ref DOCKER_NETWORK: String = env::var("DOCKER_NETWORK").unwrap_or_else(|_| String::from("host"));
    static ref DOCKER_GPUS: String = env::var("DOCKER_GPUS").unwrap_or_else(|_| String::from(""));
    static ref DOCKER_PRIVILEGED: String = env::var("DOCKER_PRIVILEGED").unwrap_or_else(|_| String::from(""));
    static ref DOCKER_VOLUME: String = env::var("DOCKER_VOLUME").unwrap_or_else(|_| String::from(""));
    static ref DOCKER_VOLUMES_FROM: String = env::var("DOCKER_VOLUMES_FROM").unwrap_or_else(|_| String::from(""));
}

#[derive(Clone, Default)]
pub struct DockerExecutor {
    pub data: Option<PathBuf>,
}

impl DockerExecutor {
    pub fn new(data: Option<PathBuf>) -> Self {
        Self { data }
    }
}

#[async_trait]
impl VmExecutor for DockerExecutor {
    /* TIM */
    /// **Edited: brought up to speed with the VmExecutor trait, which means it properly implements errors now.**
    /// 
    /// Performs a call of an external job in a local docker container.
    /// 
    /// **Arguments**  
    ///  * `function`: The external function to execute.
    ///  * `arguments`: A key/value map of parameters for the function.
    ///  * `location`: The Brane location where to execute the job. Note this is actually ignored for the DockerExecutor, since we always execute locally.
    /// 
    /// **Returns**  
    /// The Value of the call upon success, or an ExecutorError otherwise.
    async fn call(
        &self,
        function: FunctionExt,
        arguments: HashMap<String, Value>,
        location: Option<String>,
    ) -> Result<Value, ExecutorError> {
        // Try to get the package directory
        let package_dir = match get_package_dir(&function.package, Some("latest"), false) {
            Ok(res) => res,
            Err(reason) => { return Err(ExecutorError::PackageDirError{ package: function.package.clone(), err: format!("{}", reason) }); }
        };

        // Get the package info
        let package_file = package_dir.join("package.yml");
        let package_info = match PackageInfo::from_path(package_file.clone()) {
            Ok(res)     => res,
            Err(reason) => { return Err(ExecutorError::PackageInfoError{ package: function.package.clone(), path: package_file, err: reason }); }
        };

        // Let the user know that this executor ignores location
        if let Some(location) = location {
            warn!("Running locally; ignoring location '{}'", location);
        }

        // Prepare the image to load
        let image = format!("{}:{}", package_info.name, package_info.version);
        let image_file = Some(package_dir.join("image.tar"));
        debug!("External package image: {}", image_file.clone().unwrap().display());

        // Prepare the list of arguments
        debug!("Parsing arguments...");
        let arguments_json = match serde_json::to_string(&arguments) {
            Ok(args)    => args,
            Err(reason) => { return Err(ExecutorError::IllegalArguments{ args: arguments, err: reason }); }
        };

        // Prepare the command
        let command = vec![
            String::from("-d"),
            String::from("--application-id"),
            String::from("test"),
            String::from("--location-id"),
            String::from("localhost"),
            String::from("--job-id"),
            String::from("1"),
            String::from(package_info.kind),
            function.name.clone(),
            base64::encode(arguments_json),
        ];

        // Collect the mounts to add
        debug!("Collecting mount folders...");
        let mounts = if let Some(data) = &self.data {
            // Get the absolute path and make sure it exists
            let data = match std::fs::canonicalize(data) {
                Ok(path)    => path,
                Err(reason) => { return Err(ExecutorError::IllegalDataDir{ path: data.clone(), err: reason }); }
            };
            if !data.exists() { return Err(ExecutorError::DataDirDoesntExist{ path: data }); }

            // Try to format the data
            let data_path = match data.clone().into_os_string().into_string() {
                Ok(s)  => s,
                Err(_) => { return Err(ExecutorError::UnreadableDataDir{ path: data }); }
            };

            // Finally, do a check for a dot
            if data_path.contains(':') { return Err(ExecutorError::IllegalDataDirColon{ path: data }); }

            // Now return
            Some(vec![format!("{}:/data", data_path)])
        } else {
            None
        };

        // With the arguments fully prepared, run the function
        debug!("About to call docker with \"{:?}\"", command);
        let exec = ExecuteInfo::new(image, image_file, mounts, Some(command));
        if function.detached {
            // Launch the function and return a struct detailling the job

            // Launch the container and get its address
            let name = run(exec).await?;
            let address = get_container_address(&name).await?;

            // Prepare a hashmap listing the properties of the this job
            let mut properties = HashMap::default();
            properties.insert(String::from("identifier"), Value::Unicode(name));
            properties.insert(String::from("address"), Value::Unicode(address));

            // Return it
            Ok(Value::Struct {
                data_type: String::from("Service"),
                properties,
            })
        } else {
            // Launch the function and await its result

            // Launch it and wait until its completed
            let (code, stdout, stderr) = run_and_wait(exec).await?;
            debug!("return code: {}", code);
            debug!("stderr: {}", stderr);
            debug!("stdout: {}", stdout);

            // If the return code is no bueno, error and show stderr
            if code != OK_RETURN_CODE {
                return Err(ExecutorError::ExternalCallFailed{ name: function.name, package: function.package, version: function.version, code: code, stdout: stdout, stderr: stderr });
            }

            // If it went right, try to decode the output
            let output = stdout.lines().last().unwrap_or_default().to_string();
            match decode_b64(output) {
                Ok(res)     => Ok(res),
                Err(reason) => Err(ExecutorError::OutputDecodeError{ name: function.name, package: function.package, version: function.version, stdout: stdout, err: reason }),
            }
        }
    }

    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Sends a message to the debug logging channel.
    /// 
    /// **Arguments**  
    ///  * `text`: The message to send.
    /// 
    /// **Returns**  
    /// Nothing if successfull, or an ExecutorError otherwise.
    async fn debug(
        &self,
        text: String,
    ) -> Result<(), ExecutorError> {
        debug!("{}", text);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Sends a message to stderr.
    /// 
    /// **Arguments**  
    ///  * `text`: The message to send.
    /// 
    /// **Returns**  
    /// Nothing if successfull, or an ExecutorError otherwise.
    async fn stderr(
        &self,
        text: String,
    ) -> Result<(), ExecutorError> {
        eprintln!("{}", text);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Sends a message to stdout.
    /// 
    /// **Arguments**  
    ///  * `text`: The message to send.
    /// 
    /// **Returns**  
    /// Nothing if successfull, or an ExecutorError otherwise.
    async fn stdout(
        &self,
        text: String,
    ) -> Result<(), ExecutorError> {
        println!("{}", text);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Launches a new job and waits until it has reached the target ServiceState.
    /// Note that this function makes its own connection to the local Docker daemon
    /// 
    /// **Arguments**  
    ///  * `text`: The message to send.
    /// 
    /// **Returns**  
    /// Nothing if successfull, or an ExecutorError otherwise.
    async fn wait_until(
        &self,
        service: String,
        state: brane_bvm::executor::ServiceState,
    ) -> Result<(), ExecutorError> {
        // If the state is started, we always return(?) - I think to prevent deadlocks
        if let brane_bvm::executor::ServiceState::Started = state {
            return Ok(());
        }

        // Connect to docker
        let docker = match Docker::connect_with_local_defaults() {
            Ok(res)     => res,
            Err(reason) => { return Err(ExecutorError::DockerConnectionFailed{ err: reason }); }
        };

        // Wait for the container
        if let Err(reason) = docker.wait_container(&service, None::<WaitContainerOptions<String>>).try_collect::<Vec<_>>().await {
            return Err(ExecutorError::DockerWaitError{ name: service, image: "???".to_string(), err: reason });
        };

        // Done
        Ok(())
    }
    /*******/
}

/* TIM */
/// **Edited: Changed to return ExecutorErrors.**
///
/// Tries to decode the given output from Base64, as UTF-8 and then as JSON.
/// 
/// **Arguments**
///  * `input`: The input string to decode.
/// 
/// **Returns**  
/// The decoded output on success, or an ExecutorError otherwise.
fn decode_b64<T>(input: String) -> Result<T, EncodeDecodeError>
where
    T: DeserializeOwned,
{
    // First, try to decode the raw base64
    let input = match base64::decode(input) {
        Ok(bin)     => bin,
        Err(reason) => { return Err(EncodeDecodeError::Base64DecodeError{ err: reason }); }
    };

    // Next, try to decode the binary as UTF-8
    let input = match String::from_utf8(input[..].to_vec()) {
        Ok(text)    => text,
        Err(reason) => { return Err(EncodeDecodeError::Utf8DecodeError{ err: reason }); }
    };

    // Finally, try to decode the JSON
    match serde_json::from_str(&input) {
        Ok(json)    => Ok(json),
        Err(reason) => Err(EncodeDecodeError::JsonDecodeError{ err: reason }),
    }
}
/*******/

///
///
///
#[derive(Deserialize, Serialize)]
pub struct ExecuteInfo {
    pub command: Option<Vec<String>>,
    pub image: String,
    pub image_file: Option<PathBuf>,
    pub mounts: Option<Vec<String>>,
}

impl ExecuteInfo {
    ///
    ///
    ///
    pub fn new(
        image: String,
        image_file: Option<PathBuf>,
        mounts: Option<Vec<String>>,
        command: Option<Vec<String>>,
    ) -> Self {
        ExecuteInfo {
            command,
            image,
            image_file,
            mounts,
        }
    }
}

/* TIM */
/// **Edited: Changed to return ExecutorErrors.**
/// 
/// Launches the given job and returns its name so it can be tracked.
///
/// **Arguments**
///  * `exec`: The ExecuteInfo that describes the job to launch.
/// 
/// **Returns**  
/// The name of the job (from Docker) if successful, or an ExecutorError upon failure.
pub async fn run(exec: ExecuteInfo) -> Result<String, ExecutorError> {
    // Connect to docker
    let docker = match Docker::connect_with_local_defaults() {
        Ok(res)     => res,
        Err(reason) => { return Err(ExecutorError::DockerConnectionFailed{ err: reason }); }
    };

    // Either import or pull image, if not already present
    ensure_image(&docker, &exec).await?;

    // Start container, return immediately (propagating any errors that occurred)
    create_and_start_container(&docker, &exec).await
}
/*******/

/* TIM */
/// Launches the given container and waits until its completed.  
/// Note that this function makes its own connection to the local Docker daemon
///
/// **Arguments**
///  * `exec`: The ExecuteInfo describing what to launch and how.
/// 
/// **Returns**  
/// The return code of the docker container, its stdout and its stderr (in that order).
pub async fn run_and_wait(exec: ExecuteInfo) -> Result<(i32, String, String), ExecutorError> {
    // Connect to docker
    let docker = match Docker::connect_with_local_defaults() {
        Ok(res)     => res,
        Err(reason) => { return Err(ExecutorError::DockerConnectionFailed{ err: reason }); }
    };

    // Either import or pull image, if not already present
    ensure_image(&docker, &exec).await?;

    // Start container and wait for completion
    let name = create_and_start_container(&docker, &exec).await?;
    if let Err(reason) = docker.wait_container(&name, None::<WaitContainerOptions<String>>).try_collect::<Vec<_>>().await {
        return Err(ExecutorError::DockerWaitError{ name: name, image: exec.image.clone(), err: reason });
    }

    // Get stdout and stderr logs from container
    let logs_options = Some(LogsOptions::<String> {
        stdout: true,
        stderr: true,
        ..Default::default()
    });
    let log_outputs = match docker.logs(&name, logs_options).try_collect::<Vec<LogOutput>>().await {
        Ok(out)     => out,
        Err(reason) => { return Err(ExecutorError::DockerLogsError{ name: name, image: exec.image.clone(), err: reason }); }
    };

    // Collect them in one string per output channel
    let mut stderr = String::new();
    let mut stdout = String::new();
    for log_output in log_outputs {
        match log_output {
            LogOutput::StdErr { message } => stderr.push_str(String::from_utf8_lossy(&message).as_ref()),
            LogOutput::StdOut { message } => stdout.push_str(String::from_utf8_lossy(&message).as_ref()),
            _ => { continue; },
        }
    }

    // Get the container's exit status by inspecting it
    let code = returncode_container(&docker, &name).await?;

    // Don't leave behind any waste: remove container
    remove_container(&docker, &name).await?;

    // Return the return data of this container!
    Ok((code, stdout, stderr))
}
/*******/

/* TIM */
/// Returns the exit code of a container is (hopefully) already stopped.
/// 
/// **Arguments**
///  * `docker`: The Docker instance to use for accessing the container.
///  * `name`: The container's name.
/// 
/// **Returns**  
/// An Ok() with the exit code or an ExecutorError explaining why we couldn't get it.
async fn returncode_container(docker: &Docker, name: &str) -> Result<i32, ExecutorError> {
    // Do the inspect call
    let info = match docker.inspect_container(name, None).await {
        Ok(info)    => info,
        Err(reason) => { return Err(ExecutorError::DockerInspectContainerError{ name: name.to_string(), err: reason }); }
    };

    // Try to get the execution state from the container
    let state = match info.state {
        Some(state) => state,
        None        => { return Err(ExecutorError::DockerContainerNoState{ name: name.to_string() }); }
    };

    // Finally, try to get the exit code itself
    match state.exit_code {
        Some(code) => Ok(code as i32),
        None       => Err(ExecutorError::DockerContainerNoExitCode{ name: name.to_string() }),
    }
}
/*******/

/* TIM */
/// **Edited: Changed to return ExecutorErrors.**
///
/// Creates a container with the given image and starts it (non-blocking after that).
/// 
/// **Arguments**
///  * `docker`: The Docker instance to use for accessing the container.
///  * `exec`: The ExecuteInfo describing what to launch and how.
/// 
/// **Returns**  
/// The name of the started container if successfull, or an ExecutorError otherwise.
async fn create_and_start_container(
    docker: &Docker,
    exec: &ExecuteInfo,
) -> Result<String, ExecutorError> {
    // Generate unique (temporary) container name
    let name = Uuid::new_v4().to_string().chars().take(8).collect::<String>();
    let create_options = CreateContainerOptions { name: &name };

    // Add any requested devices (GPUs)
    let device_requests = if DOCKER_GPUS.as_str() != "" {
        let device_request = DeviceRequest {
            driver: None,
            count: Some(-1),
            device_ids: None,
            capabilities: Some(vec![vec![String::from("gpu")]]),
            options: None,
        };

        Some(vec![device_request])
    } else {
        None
    };

    // Add any volumes
    let volumes_from = if DOCKER_VOLUMES_FROM.as_str() != "" {
        Some(vec![DOCKER_VOLUMES_FROM.to_string()])
    } else {
        None
    };

    // Add the brane volume if we need one
    let mut binds = if DOCKER_VOLUME.as_str() != "" {
        vec![format!("{}:/brane", DOCKER_VOLUME.as_str())]
    } else {
        exec.mounts.clone().unwrap_or_default()
    };

    // Add the docker socket
    binds.push(String::from("/var/run/docker.sock:/var/run/docker.sock"));

    // Combine the properties
    let host_config = HostConfig {
        binds: Some(binds),
        network_mode: Some(DOCKER_NETWORK.to_string()),
        privileged: Some(DOCKER_PRIVILEGED.as_str() == "true"),
        volumes_from,
        device_requests,
        ..Default::default()
    };

    // Create the container confic
    let create_config = Config {
        image: Some(exec.image.clone()),
        cmd: exec.command.clone(),
        host_config: Some(host_config),
        ..Default::default()
    };

    if let Err(reason) = docker.create_container(Some(create_options), create_config).await { return Err(ExecutorError::DockerCreateContainerError{ name: name, image: exec.image.clone(), err: reason }); }
    match docker.start_container(&name, None::<StartContainerOptions<String>>).await {
        Ok(_)       => Ok(name),
        Err(reason) => Err(ExecutorError::DockerStartError{ name: name, image: exec.image.clone(), err: reason })
    }
}

/* TIM */
/// **Edited: Now returns ExecutorErrors.**
///
/// Tries to import/pull the given image if it does not exist in the local Docker instance.
/// 
/// **Arguments**
///  * `docker`: An already connected local instance of Docker.
///  * `exec`: The ExecuteInfo describing the image to pull.
/// 
/// **Returns**  
/// Nothing on success (whether we imported/pulled it or found it already existed), but an ExecutorError upon failure.
async fn ensure_image(
    docker: &Docker,
    exec: &ExecuteInfo,
) -> Result<(), ExecutorError> {
    // Abort if image is already loaded
    if docker.inspect_image(&exec.image).await.is_ok() {
        debug!("Image already exists in Docker deamon.");
        return Ok(());
    }

    // Otherwise, import it if it is described or pull it
    if let Some(image_file) = &exec.image_file {
        debug!("Image doesn't exist in Docker deamon: importing...");
        import_image(docker, image_file).await
    } else {
        debug!("Image '{}' doesn't exist in Docker deamon: pulling...", exec.image);
        pull_image(docker, exec.image.clone()).await
    }
}
/*******/

/* TIM */
/// **Edited: Now returns ExecutorErrors.**
///
/// Tries to import the image at the given path into the given Docker instance.
/// 
/// **Arguments**
///  * `docker`: An already connected local instance of Docker.
///  * `image_file`: Path to the image to import.
/// 
/// **Returns**  
/// Nothing on success, or an ExecutorError otherwise.
async fn import_image(
    docker: &Docker,
    image_file: &Path,
) -> Result<(), ExecutorError> {
    let options = ImportImageOptions { quiet: true };

    // Try to read the file
    let file = match TFile::open(image_file).await {
        Ok(handle)  => handle,
        Err(reason) => { return Err(ExecutorError::ImageReadError{ path: PathBuf::from(image_file), err: reason }); }
    };

    // If successful, open the byte with a FramedReader, freezing all the chunk we read
    let byte_stream = FramedRead::new(file, BytesCodec::new()).map(|r| {
        let bytes = r.unwrap().freeze();
        Ok::<_, Error>(bytes)
    });

    // Finally, wrap it in a HTTP body and send it to the Docker API
    let body = Body::wrap_stream(byte_stream);
    match docker.import_image(options, body, None).try_collect::<Vec<_>>().await {
        Ok(_)       => Ok(()),
        Err(reason) => Err(ExecutorError::DockerImportError{ path: PathBuf::from(image_file), err: reason })
    }
}
/*******/

/* TIM */
/// **Edited: Now returns ExecutorErrors.**
///
/// Pulls a new image from the given Docker image ID / URL (?) and imports it in the Docker instance.
/// 
/// **Arguments**
///  * `docker`: An already connected local instance of Docker.
///  * `image`: The image to pull.
/// 
/// **Returns**  
/// Nothing on success, or an ExecutorError otherwise.
async fn pull_image(
    docker: &Docker,
    image: String,
) -> Result<(), ExecutorError> {
    // Define the options for this image
    let options = Some(CreateImageOptions {
        from_image: image.clone(),
        ..Default::default()
    });

    // Try to create it
    match docker.create_image(options, None, None).try_collect::<Vec<_>>().await {
        Ok(_)       => Ok(()),
        Err(reason) => Err(ExecutorError::DockerCreateImageError{ image: image, err: reason }),
    }
}
/*******/

/* TIM */
/// *Edited: Now returns ExecutorErrors.**
///
/// Tries to remove the docker container with the given name.
/// 
/// **Arguments**
///  * `docker`: An already connected local instance of Docker.
///  * `name`: The name of the container to remove.
/// 
/// **Returns**  
/// Nothing on success, or an ExecutorError otherwise.
async fn remove_container(
    docker: &Docker,
    name: &str,
) -> Result<(), ExecutorError> {
    // Set the options
    let remove_options = Some(RemoveContainerOptions {
        force: true,
        ..Default::default()
    });

    // Attempt the removal
    match docker.remove_container(name, remove_options).await {
        Ok(_)       => Ok(()),
        Err(reason) => Err(ExecutorError::DockerRemoveContainerError{ name: name.to_string(), err: reason }),
    }
}
/*******/

/* TIM */
/// *Edited: Now returns ExecutorErrors.**
///
/// Tries to remove the docker image with the given name.  
/// Note that this function makes a separate connection to the local Docker instance.
/// 
/// **Arguments**
///  * `name`: The name of the image to remove.
/// 
/// **Returns**  
/// Nothing on success, or an ExecutorError otherwise.
pub async fn remove_image(name: &str) -> Result<(), ExecutorError> {
    // Try to connect to the local instance
    let docker = match Docker::connect_with_local_defaults() {
        Ok(conn)    => conn,
        Err(reason) => { return Err(ExecutorError::DockerConnectionFailed{ err: reason }); }
    };

    // Check if the image still exists
    let image = docker.inspect_image(name).await;
    if image.is_err() {
        // It doesn't, easy
        return Ok(());
    }

    // Set the options to remove
    let remove_options = Some(RemoveImageOptions {
        force: true,
        ..Default::default()
    });

    // Now we can try to remove the image
    let image = image.unwrap();
    match docker.remove_image(&image.id, remove_options, None).await {
        Ok(_)       => Ok(()),
        Err(reason) => Err(ExecutorError::DockerRemoveImageError{ name: name.to_string(), id: image.id.clone(), err: reason }),
    }
}
/*******/

/* TIM */
/// *Edited: Now returns ExecutorErrors.**
///
/// Tries to return the address of the container with the given name.  
/// Note that this function makes a separate connection to the local Docker instance.
/// 
/// **Arguments**
///  * `name`: The name of the image to remove.
/// 
/// **Returns**  
/// The address of the container as a string on success, or an ExecutorError otherwise.
pub async fn get_container_address(name: &str) -> Result<String, ExecutorError> {
    // Try to connect to the local instance
    let docker = match Docker::connect_with_local_defaults() {
        Ok(conn)    => conn,
        Err(reason) => { return Err(ExecutorError::DockerConnectionFailed{ err: reason }); }
    };

    // Try to inspect the container
    let container = match docker.inspect_container(name, None).await {
        Ok(data)    => data,
        Err(reason) => { return Err(ExecutorError::DockerInspectContainerError{ name: name.to_string(), err: reason }); }
    };

    // Get the networks of this container
    let networks = container
        .network_settings
        .map(|n| n.networks)
        .flatten()
        .unwrap_or_default();

    // Next, get the address of the first network and try to return that
    if let Some(network) = networks.values().next() {
        let ip = network.ip_address.clone().unwrap_or_default();
        if ip.is_empty() {
            Ok(String::from("127.0.0.1"))
        } else {
            Ok(ip)
        }
    } else {
        Err(ExecutorError::DockerContainerNoNetwork{ name: name.to_string() })
    }
}
/*******/
