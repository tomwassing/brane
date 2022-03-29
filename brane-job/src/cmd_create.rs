use crate::errors::JobError;
use crate::interface::{Command, CommandKind, Event, EventKind};
use anyhow::Result;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::image::CreateImageOptions;
use bollard::models::HostConfig;
use bollard::Docker;
use brane_cfg::infrastructure::{Location, LocationCredentials};
use brane_cfg::{Infrastructure, Secrets};
use dashmap::lock::RwLock;
use dashmap::DashMap;
use futures_util::stream::TryStreamExt;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{Api, PostParams};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Client as KubeClient, Config as KubeConfig};
use rand::distributions::Alphanumeric;
use rand::{self, Rng};
use serde_json::{json, Value as JValue};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::iter;
use std::sync::Arc;
use xenon::compute::{JobDescription, Scheduler};
use xenon::credentials::{CertificateCredential, Credential};
use xenon::storage::{FileSystem, FileSystemPath};

// Names of environment variables.
const BRANE_APPLICATION_ID: &str = "BRANE_APPLICATION_ID";
const BRANE_LOCATION_ID: &str = "BRANE_LOCATION_ID";
const BRANE_JOB_ID: &str = "BRANE_JOB_ID";
const BRANE_CALLBACK_TO: &str = "BRANE_CALLBACK_TO";
const BRANE_PROXY_ADDRESS: &str = "BRANE_PROXY_ADDRESS";
const BRANE_MOUNT_DFS: &str = "BRANE_MOUNT_DFS";

/* TIM */
/// **Edited: now returning JobErrors.**
/// 
/// Handles an incoming CREATE command.
/// 
/// **Arguments**
///  * `key`: The key of the message that brought us the command.
///  * `command`: The Command struct that contains the message payload, already parsed.
///  * `infra`: The Infrastructure handle to the infra.yml.
///  * `secrets`: The Secrets handle to the infra.yml.
///  * `xenon_endpoint`: The Xenon endpoint to connect to and schedule jobs on.
///  * `xenon_schedulers`: A list of Xenon schedulers we use to determine where to run what.
/// 
/// **Returns**  
/// A list of events to fire on success, or else a JobError listing what went wrong.
pub async fn handle(
    key: &str,
    mut command: Command,
    infra: Infrastructure,
    secrets: Secrets,
    xenon_endpoint: String,
    xenon_schedulers: Arc<DashMap<String, Arc<RwLock<Scheduler>>>>,
) -> Result<Vec<(String, Event)>, JobError> {
    // Get some stuff from the command struct first
    debug!("Validating CREATE command...");
    validate_command(key, &command)?;
    let application = command.application.clone().unwrap();
    let correlation_id = command.identifier.clone().unwrap();
    let image = command.image.clone().unwrap();

    // Retreive location metadata and credentials.
    debug!("Retrieving location data...");
    let location_id = command.location.clone().unwrap();
    let location = match infra.get_location_metadata(&location_id) {
        Ok(location) => location,
        Err(reason)  => { return Err(JobError::InfrastructureError{ err: reason }); }
    };

    // Get the image
    // command.image = Some(format!("{}/library/{}", location.get_registry(), &image)); // Removed cause this caused double registry in URL
    command.image = Some(image.to_string());

    // Generate job identifier.
    let job_id = format!("{}-{}", correlation_id, get_random_identifier());

    // Next, handle the location
    match handle_location(
        &application,
        &correlation_id,
        &job_id,
        &location_id,
        location,
        command,
        secrets,
        xenon_endpoint,
        xenon_schedulers,
    ).await {
        Ok(events) => Ok(events),
        Err(err) => {
            // Convert these errors to CreateFailed events too
            // The error becomes the payload
            let payload = format!("{}", err).into_bytes();

            // Construct the event object
            let category = String::from("job");
            let order = 0; // A CREATE event is always the first, thus order=0.
            let event = Event::new(
                EventKind::CreateFailed,
                job_id.clone(),
                application,
                location_id,
                category,
                order,
                Some(payload),
                None,
            );

            // Return the list with this event
            let key = format!("{}#{}", job_id, order);
            Ok(vec!((key, event)))
        }
    }
}



/// Schedules the actual job on the given location
/// 
/// **Arguments**
///  * `application`: The name of the application for which we schedule the job.
///  * `correlation_id`: The driver-assigned correlation ID for this job.
///  * `job_id`: The ID of this job.
///  * `location_id`: The ID of the location where the job will be scheduled.
///  * `location`: The metadata of the location where the job will be scheduled.
///  * `command`: The actual command to run.
///  * `secrets`: Handle to the secrets.yml with secrets.
///  * `xenon_endpoint`: The Xenon endpoint to connect to and schedule jobs on.
///  * `xenon_schedulers`: A list of Xenon schedulers we use to determine where to run what.
#[allow(clippy::too_many_arguments)]
async fn handle_location(
    application_id: &str,
    correlation_id: &str,
    job_id: &str,
    location_id: &str,
    location: Location,
    command: Command,
    secrets: Secrets,
    xenon_endpoint: String,
    xenon_schedulers: Arc<DashMap<String, Arc<RwLock<Scheduler>>>>,
) -> Result<Vec<(String, Event)>, JobError> {
    // Get the image from the command
    let image = command.image.clone().unwrap();

    // Branch into specific handlers based on the location kind.
    match location {
        Location::Kube {
            address,
            callback_to,
            namespace,
            credentials,
            proxy_address,
            mount_dfs,
            ..
        } => {
            debug!("Executing command in Kubernetes environment...");
            let environment = construct_environment(
                application_id,
                location_id,
                job_id,
                &callback_to,
                &proxy_address,
                &mount_dfs,
            )?;
            let credentials = credentials.resolve_secrets(&secrets);

            handle_k8s(command, job_id, location_id, environment, address, namespace, credentials).await?
        }
        Location::Local {
            callback_to,
            network,
            proxy_address,
            mount_dfs,
            ..
        } => {
            debug!("Executing command locally with network '{}'...", network);
            let environment = construct_environment(
                application_id,
                location_id,
                job_id,
                &callback_to,
                &proxy_address,
                &mount_dfs,
            )?;
            handle_local(command, correlation_id, location_id, environment, network).await?
        }
        Location::Slurm {
            address,
            callback_to,
            runtime,
            credentials,
            proxy_address,
            mount_dfs,
            ..
        } => {
            debug!("Executing command using slurm...");
            let environment = construct_environment(
                application_id,
                location_id,
                job_id,
                &callback_to,
                &proxy_address,
                &mount_dfs,
            )?;
            let credentials = credentials.resolve_secrets(&secrets);

            handle_slurm(
                command,
                job_id,
                location_id,
                environment,
                address,
                runtime,
                credentials,
                xenon_endpoint,
                xenon_schedulers,
            )
            .await?
        }
        Location::Vm {
            address,
            callback_to,
            runtime,
            credentials,
            proxy_address,
            mount_dfs,
            ..
        } => {
            debug!("Executing command on Brane VM...");
            let environment = construct_environment(
                application_id,
                location_id,
                job_id,
                &callback_to,
                &proxy_address,
                &mount_dfs,
            )?;
            let credentials = credentials.resolve_secrets(&secrets);

            handle_vm(
                command,
                job_id,
                location_id,
                environment,
                address,
                runtime,
                credentials,
                xenon_endpoint,
                xenon_schedulers,
            )
            .await?
        }
    };

    info!(
        "Created job '{}' at location '{}' as part of application '{}'.",
        job_id, location_id, application_id
    );

    // Extract the digest from the image, if any
    let image: &str = if image.contains('@') {
        &image[..image.find('@').unwrap()]
    } else {
        &image
    };

    let order = 0; // A CREATE event is always the first, thus order=0.
    let key = format!("{}#{}", job_id, order);
    let category = String::from("job");
    let payload = image.to_string().into_bytes();
    let event = Event::new(
        EventKind::Created,
        job_id.to_string(),
        application_id.to_string(),
        location_id.to_string(),
        category,
        order,
        Some(payload),
        None,
    );

    Ok(vec![(key, event)])
}
/*******/

/* TIM */
/// **Edited: now returning JobError. Also taking the message key.**
/// 
/// Validates if the necessary fields are populated in the given Command struct.
/// 
/// **Arguments**
///  * `key`: The key of the Command's original message (use for debugging)
///  * `command`: The Command instance to validate.
/// 
/// **Returns**  
/// Nothing if the command was a-okay, or else a JobError.
fn validate_command(key: &str, command: &Command) -> Result<(), JobError> {
    if command.identifier.is_none()  { return Err(JobError::IllegalCommandError{ key: key.to_string(), kind: format!("{}", CommandKind::from_i32(command.kind).unwrap()), field: "identifier".to_string() }); }
    if command.application.is_none() { return Err(JobError::IllegalCommandError{ key: key.to_string(), kind: format!("{}", CommandKind::from_i32(command.kind).unwrap()), field: "application".to_string() }); }
    if command.location.is_none()    { return Err(JobError::IllegalCommandError{ key: key.to_string(), kind: format!("{}", CommandKind::from_i32(command.kind).unwrap()), field: "location".to_string() }); }
    if command.image.is_none()       { return Err(JobError::IllegalCommandError{ key: key.to_string(), kind: format!("{}", CommandKind::from_i32(command.kind).unwrap()), field: "image".to_string() }); }
    Ok(())
}
/*******/

/* TIM */
/// **Edited: now returning JobErrors.**
/// 
/// Creates the environment map with the given properties.
/// 
/// **Arguments**
///  * `application_id`: The ID of the current application we're treating.
///  * `location_id`: The ID of the location where we'll run.
///  * `job_id`: The ID of this job.
///  * `callback_to`: The channel to callback to during job execution.
///  * `proxy_address`: Address of a proxy to use, if any.
///  * `mount_dfs`: The path to the dynamic, global filesystem, if any.
/// 
/// **Returns**  
/// A map with the environment variables on success, or a JobError otherwise.
fn construct_environment<S: Into<String>>(
    application_id: S,
    location_id: S,
    job_id: S,
    callback_to: S,
    proxy_address: &Option<String>,
    mount_dfs: &Option<String>,
) -> Result<HashMap<String, String>, JobError> {
    let mut environment = hashmap! {
        BRANE_APPLICATION_ID.to_string() => application_id.into(),
        BRANE_LOCATION_ID.to_string() => location_id.into(),
        BRANE_JOB_ID.to_string() => job_id.into(),
        BRANE_CALLBACK_TO.to_string() => callback_to.into(),
    };

    if let Some(proxy_address) = proxy_address {
        environment.insert(BRANE_PROXY_ADDRESS.to_string(), proxy_address.clone());
    }

    if let Some(mount_dfs) = mount_dfs {
        environment.insert(BRANE_MOUNT_DFS.to_string(), mount_dfs.clone());
    }

    Ok(environment)
}
/*******/





/***** KUBERNETES *****/
/* TIM */
/// **Edited: now returning JobErrors + accepting location ID.**
/// 
/// Schedules the job on a Kubernetes cluster.
/// 
/// **Arguments**
///  * `command`: The Command to schedule.
///  * `job_id`: The ID of this job.
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `environment`: The environment to set for the job.
///  * `address`: The address of the target Kubernetes control plane. (ignored?)
///  * `namespace`: The Kubernetes namespace for this job.
///  * `credentials`: The relevant LocationCredentials for the Kubernetes cluster.
/// 
/// **Returns**  
/// Nothing on success, or else a JobError describing what went wrong.
async fn handle_k8s(
    command: Command,
    job_id: &str,
    location_id: &str,
    environment: HashMap<String, String>,
    _address: String,
    namespace: String,
    credentials: LocationCredentials,
) -> Result<(), JobError> {
    // Create Kubernetes client based on config credentials
    let client = match credentials {
        LocationCredentials::Config { file } => {
            let config = construct_k8s_config(location_id, file).await?;
            match KubeClient::try_from(config) {
                Ok(client)  => client,
                Err(reason) => { return Err(JobError::K8sClientError{ location_id: location_id.to_string(), err: reason }); },
            }
        },
        cred => { return Err(JobError::K8sIllegalCredentials{ location_id: location_id.to_string(), cred_type: cred.cred_type().to_string() }); }
    };

    // Create the job description
    let job_description = create_k8s_job_description(job_id, location_id, &command, environment)?;

    // Try to run it!
    let jobs: Api<Job> = Api::namespaced(client.clone(), &namespace);
    let result = jobs.create(&PostParams::default(), &job_description).await;

    // Try again if job creation failed because of missing namespace.
    if let Err(error) = result {
        match error {
            kube::Error::Api(error) => {
                if error.message.starts_with("namespaces") && error.reason.as_str() == "NotFound" {
                    warn!(
                        "Failed to create k8s job because namespace '{}' didn't exist.",
                        namespace
                    );

                    // First create namespace
                    let namespaces: Api<Namespace> = Api::all(client.clone());
                    let new_namespace = create_k8s_namespace(location_id, &namespace)?;
                    let result = namespaces.create(&PostParams::default(), &new_namespace).await;

                    // Only try again if namespace creation succeeded.
                    if result.is_ok() {
                        info!("Created k8s namespace '{}'. Trying again to create k8s job.", namespace);
                        if let Err(reason) = jobs.create(&PostParams::default(), &job_description).await {
                            return Err(JobError::K8sCreateJobError{ job_id: job_id.to_string(), location_id: location_id.to_string(), err: reason });
                        }
                    }
                }
            }
            _ => { return Err(JobError::K8sCreateJobError{ job_id: job_id.to_string(), location_id: location_id.to_string(), err: error }); },
        }
    }

    // Done!
    Ok(())
}
/*******/

/* TIM */
/// **Edited: now returning JobErrors + requesting location ID from caller.**
/// 
/// Creates the configuration object for the Kubernetes cluster we want to run a job on.
/// 
/// **Arguments**
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `config_file`: The raw file contents of the configuration file we want to convert into a KubeConfig object.
/// 
/// **Returns**  
/// A KubeConfig object if everything went alright, or a JobError if it didn't.
async fn construct_k8s_config(location_id: &str, config_file: String) -> Result<KubeConfig, JobError> {
    let base64_symbols = ['+', '/', '='];

    // Remove any whitespace and/or newlines.
    let config_file: String = config_file
        .chars()
        .filter(|c| c.is_alphanumeric() || base64_symbols.contains(c))
        .collect();

    // Decode as Base64.
    let config_file = match base64::decode(config_file) {
        Ok(config_file) => config_file,
        Err(reason)     => { return Err(JobError::K8sBase64Error{ location_id: location_id.to_string(), err: reason }); }
    };
    // Decode as UTF-8
    let config_file = match String::from_utf8(config_file) {
        Ok(config_file) => config_file,
        Err(reason)     => { return Err(JobError::K8sUTF8Error{ location_id: location_id.to_string(), err: reason }); }
    };
    // Parse as YAML
    let config_file: Kubeconfig = match serde_yaml::from_str(&config_file) {
        Ok(config_file) => config_file,
        Err(reason)     => { return Err(JobError::K8sYAMLError{ location_id: location_id.to_string(), err: reason }); }
    };

    // Finally, throw to a real KubeConfig object
    match KubeConfig::from_custom_kubeconfig(config_file, &KubeConfigOptions::default()).await {
        Ok(config_file) => Ok(config_file),
        Err(reason)     => Err(JobError::K8sConfigError{ location_id: location_id.to_string(), err: reason }),
    }
}
/*******/

/* TIM */
/// **Edited: now returning JobErrors + requesting location ID from caller.**
///
/// Creates a job description based on the given job and environment.
/// 
/// **Arguments**
///  * `job_id`: The ID of this job.
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `command`: The Command to schedule.
///  * `environment`: The environment to set for the job.
/// 
/// **Returns**  
/// A KubeConfig object if everything went alright, or a JobError if it didn't.
fn create_k8s_job_description(
    job_id: &str,
    location_id: &str,
    command: &Command,
    environment: HashMap<String, String>,
) -> Result<Job, JobError> {
    let command = command.clone();
    let environment: Vec<JValue> = environment
        .iter()
        .map(|(k, v)| json!({ "name": k, "value": v }))
        .collect();

    // Kubernetes jobs require lowercase names
    let job_id = job_id.to_lowercase();

    // Strip the digest from the image
    let image: &str = command.image.as_ref().expect("Missing image after successful validation of Command; this should never happen!");
    let image = if image.contains('@') {
        &image[..image.find('@').unwrap()]
    } else {
        image
    };

    // Create tje JSON job description
    match serde_json::from_value(json!({
        "apiVersion": "batch/v1",
        "kind": "Job",
        "metadata": {
            "name": job_id,
        },
        "spec": {
            "backoffLimit": 3,
            "ttlSecondsAfterFinished": 120,
            "template": {
                "spec": {
                    "containers": [{
                        "name": job_id,
                        "image": image,
                        "args": command.command,
                        "env": environment,
                        "securityContext": {
                            "capabilities": {
                                "drop": ["all"],
                                "add": ["NET_BIND_SERVICE", "NET_ADMIN", "SYS_ADMIN"]
                            },
                            "privileged": true // Quickfix, needs to be dynamic based on capabilities/devices used.
                        }
                    }],
                    "restartPolicy": "Never",
                }
            }
        }
    }))
    {
        Ok(job_description) => Ok(job_description),
        Err(reason)         => Err(JobError::K8sJobDescriptionError{ job_id, location_id: location_id.to_string(), err: reason }),
    }
}
/*******/

/* TIM */
/// **Edited: now returning JobErrors.**
/// 
/// Attempts to create a Kubernetes namespace.
/// 
/// **Arguments**
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `namespace`: The namespace name we want to create.
/// 
/// **Returns**  
/// The new namespace as a Namespace object on success, or a JobError with the error otherwise.
fn create_k8s_namespace(location_id: &str, namespace: &str) -> Result<Namespace, JobError> {
    match serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": {
            "name": namespace,
        }
    }))
    {
        Ok(namespace) => Ok(namespace),
        Err(reason)   => Err(JobError::K8sNamespaceError{ location_id: location_id.to_string(), namespace: namespace.to_string(), err: reason }),
    }
}
/*******/





/***** LOCAL *****/
/* TIM */
/// **Edited: now returning JobErrors + accepting location ID.**
/// 
/// Schedules the job on a local Docker instance.
/// 
/// **Arguments**
///  * `command`: The Command to schedule.
///  * `job_id`: The ID of this job.
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `environment`: The environment to set for the job.
///  * `network`: The Docker network name to use for this job.
/// 
/// **Returns**  
/// Nothing on success, or else a JobError describing what went wrong.
async fn handle_local(
    command: Command,
    job_id: &str,
    _location_id: &str,
    environment: HashMap<String, String>,
    network: String,
) -> Result<(), JobError> {
    let docker = match Docker::connect_with_local_defaults() {
        Ok(docker)  => docker,
        Err(reason) => { return Err(JobError::DockerConnectionFailed{ err: reason }); }
    };

    debug!("Ensuring docker image...");
    let image = command.image.expect("Empty `image` field on CREATE command.");
    ensure_image(&docker, &image).await?;

    debug!("Generating docker configuration...");
    let create_options = CreateContainerOptions { name: job_id };

    let host_config = HostConfig {
        auto_remove: Some(true),
        // NOTE: Enable when the job container is doing funky
        // auto_remove: Some(false),
        network_mode: Some(network),
        privileged: Some(true),
        ..Default::default()
    };

    let environment = environment
        .iter()
        .map(|(key, value)| format!("{}={}", key, value))
        .collect();

    // Extract the digest from the image, if any
    let image: &str = if image.contains('@') {
        &image[..image.find('@').unwrap()]
    } else {
        &image
    };

    let create_config = Config {
        cmd: Some(command.command),
        env: Some(environment),
        host_config: Some(host_config),
        image: Some(image.to_string()),
        ..Default::default()
    };

    // Create and start container
    debug!("Creating docker container...");
    if let Err(err) = docker.create_container(Some(create_options), create_config).await {
        return Err(JobError::DockerCreateContainerError{ name: job_id.to_string(), image: image.to_string(), err });
    }

    debug!("Starting docker container...");
    match docker.start_container(job_id, None::<StartContainerOptions<String>>).await {
        Ok(_)    => Ok(()),
        Err(err) => Err(JobError::DockerStartError{ name: job_id.to_string(), image: image.to_string(), err }),
    }
}
/*******/

/* TIM */
/// **Edited: now returning Docker errors.**
/// 
/// Makes sure the given image is imported into the given Docker daemon.
/// 
/// **Arguments**
///  * `docker`: The Docker instance to import the images into.
///  * `image`: The Docker Image to import.
/// 
/// **Returns**  
/// Nothing on success, but a JobError on failure.
async fn ensure_image(
    docker: &Docker,
    image: &str,
) -> Result<(), JobError> {
    // Abort, if image is already loaded
    debug!("Checking if image '{}' already exists...", image);
    if docker.inspect_image(image).await.is_ok() {
        debug!("Image already exists in Docker deamon.");
        return Ok(());
    }

    // Extract the digest from the image, if any
    let image: &str = if image.contains('@') {
        &image[..image.find('@').unwrap()]
    } else {
        image
    };

    debug!("Creating image options...");
    let options = Some(CreateImageOptions {
        from_image: image,
        ..Default::default()
    });

    debug!("Creating image with options '{:?}'...", options);
    match docker.create_image(options, None, None).try_collect::<Vec<_>>().await {
        Ok(_)       => Ok(()),
        Err(reason) => Err(JobError::DockerCreateImageError{ image: image.to_string(), err: reason }),
    }
}
/*******/





/***** SLURM *****/
/* TIM */
/// **Edited: now returning JobErrors + accepting location ID.**
/// 
/// Schedules the job on the local Slurm cluster manager.
/// 
/// **Arguments**
///  * `command`: The Command to schedule.
///  * `job_id`: The ID of this job.
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `environment`: The environment to set for the job.
///  * `address`: The address of the target Xenon control plane.
///  * `credentials`: The relevant LocationCredentials for the Xenon cluster.
///  * `xenon_endpoint`: The Xenon endpoint to connect to and schedule jobs on.
///  * `xenon_schedulers`: A list of Xenon schedulers we use to determine where to run what.
/// 
/// **Returns**  
/// Nothing upon success, but a JobError describing what went wrong on failure.
#[allow(clippy::too_many_arguments)]
async fn handle_slurm(
    command: Command,
    job_id: &str,
    location_id: &str,
    environment: HashMap<String, String>,
    address: String,
    runtime: String,
    credentials: LocationCredentials,
    xenon_endpoint: String,
    xenon_schedulers: Arc<DashMap<String, Arc<RwLock<Scheduler>>>>,
) -> Result<(), JobError> {
    // Resolve the credentials
    let credentials = match credentials {
        LocationCredentials::SshCertificate {
            username,
            certificate,
            passphrase,
        } => Credential::new_certificate(certificate, username, passphrase.unwrap_or_default()),
        LocationCredentials::SshPassword { username, password } => Credential::new_password(username, password),
        credentials => { return Err(JobError::SlurmIllegalCredentials{ location_id: location_id.to_string(), cred_type: credentials.cred_type().to_string() }) },
    };

    // Create the Xenon scheduler
    let scheduler = create_xenon_scheduler(
        location_id,
        "slurm",
        address,
        credentials,
        xenon_endpoint,
        xenon_schedulers,
    ).await?;

    // Do the rest via this scheduler
    handle_xenon(command, job_id, location_id, environment, runtime, scheduler).await
}
/*******/





/***** VM *****/
/* TIM */
/// **Edited: now returning JobErrors + accepting location ID.**
/// 
/// Schedules the job on a local VM, via SSH, via Xenon.
/// 
/// **Arguments**
///  * `command`: The Command to schedule.
///  * `job_id`: The ID of this job.
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `environment`: The environment to set for the job.
///  * `address`: The address of the target Xenon control plane.
///  * `runtime`: The runtime to run the images with (either Docker or Singularity).
///  * `credentials`: The relevant LocationCredentials for the Xenon cluster.
///  * `xenon_endpoint`: The Xenon endpoint to connect to and schedule jobs on.
///  * `xenon_schedulers`: A list of Xenon schedulers we use to determine where to run what.
/// 
/// **Returns**  
/// Returns nothing on success, or else a JobError on failure.
#[allow(clippy::too_many_arguments)]
async fn handle_vm(
    command: Command,
    job_id: &str,
    location_id: &str,
    environment: HashMap<String, String>,
    address: String,
    runtime: String,
    credentials: LocationCredentials,
    xenon_endpoint: String,
    xenon_schedulers: Arc<DashMap<String, Arc<RwLock<Scheduler>>>>,
) -> Result<(), JobError> {
    // Resolve the credentials
    let credentials = match credentials {
        LocationCredentials::SshCertificate {
            username,
            certificate,
            passphrase,
        } => Credential::new_certificate(certificate, username, passphrase.unwrap_or_default()),
        LocationCredentials::SshPassword { username, password } => Credential::new_password(username, password),
        LocationCredentials::Config { .. } => unreachable!(),
    };

    // Create the scheduler to use
    let scheduler = create_xenon_scheduler(
        location_id,
        "ssh",
        address,
        credentials,
        xenon_endpoint,
        xenon_schedulers,
    ).await?;

    // Leave the rest as a normal Xenon job
    handle_xenon(command, job_id, location_id, environment, runtime, scheduler).await
}





/***** XENON *****/
/* TIM */
/// **Edited: now returning JobErrors + accepting location ID.**
/// 
/// Schedules the job on the local Xenon manager.  
/// Note that the user cannot directly choose this site; instead, it's used for both Slurm and SSH access.
/// 
/// **Arguments**
///  * `command`: The Command to schedule.
///  * `job_id`: The ID of this job.
///  * `location_id`: The ID of the location for which we construct the config. Only used for debugging purposes.
///  * `environment`: The environment to set for the job.
///  * `runtime`: The runtime to run the images with (either Docker or Singularity).
///  * `scheduler`: The Xenon scheduler that will be used to schedule the job.
/// 
/// **Returns**  
/// Nothing on success, or a JobError otherwise.
async fn handle_xenon(
    command: Command,
    job_id: &str,
    location_id: &str,
    environment: HashMap<String, String>,
    runtime: String,
    scheduler: Arc<RwLock<Scheduler>>,
) -> Result<(), JobError> {
    debug!("Handling incoming Xenon job '{}'...", job_id);
    let job_description = match runtime.to_lowercase().as_str() {
        "singularity" => create_singularity_job_description(&command, job_id, environment),
        "docker" => create_docker_job_description(&command, job_id, environment, None),
        runtime => { return Err(JobError::XenonUnknownRuntime{ runtime: runtime.to_string(), location_id: location_id.to_string() }); },
    };

    debug!("Scheduling job '{}' on Xenon...", job_id);
    if let Err(err) = scheduler.write().submit_batch_job(job_description).await {
        return Err(JobError::XenonSubmitError{ job_id: job_id.to_string(), adaptor: runtime.to_lowercase(), location_id: location_id.to_string(), err });
    };
    debug!("Job complete.");

    Ok(())
}
/*******/

/* TIM */
/// **Edited: now returning JobErrors.**
/// 
/// Creates a Xenon scheduler and returns it.
/// 
/// **Arguments**
///  * `location_id`: The location where to schedule.
///  * `adaptor`: The adaptor to use (for us, either Slurm or SSH)
///  * `location`: The location of the Xenon instance.
///  * `credential`: The Credential needed to reach the other location
///  * `xenon_endpoint`: The Xenon endpoint to connect to and schedule jobs on.
///  * `xenon_schedulers`: A list of Xenon schedulers we use to determine where to run what.
/// 
/// **Returns**  
/// The Xenon scheduler as an object, wrap in thread-safe constructs Arc and RwLock. Upon a failure, returns a JobError instead.
async fn create_xenon_scheduler<S1, S2, S3>(
    location_id: &str,
    adaptor: S2,
    location: S1,
    credential: Credential,
    xenon_endpoint: S3,
    xenon_schedulers: Arc<DashMap<String, Arc<RwLock<Scheduler>>>>,
) -> Result<Arc<RwLock<Scheduler>>, JobError>
where
    S1: Into<String>,
    S2: Into<String>,
    S3: Into<String>,
{
    // Check if we have already created a scheduler for this location
    if xenon_schedulers.contains_key(location_id) {
        let scheduler = xenon_schedulers.get(location_id).unwrap();
        let scheduler = scheduler.value();

        // Check if the scheduler is still writeable
        let is_open = match scheduler.write().is_open().await {
            Ok(is_open) => is_open,
            Err(err)    => { return Err(JobError::XenonIsOpenError{ location_id: location_id.to_string(), err }); }
        };
        if is_open {
            // We can return it!
            return Ok(scheduler.clone());
        } else {
            // We'll need to re-create it anyway
            xenon_schedulers.remove(location_id);
        }
    }

    // Convert all string-likes into strings
    let adaptor = adaptor.into();
    let location = location.into();
    let xenon_endpoint = xenon_endpoint.into();

    // Define the properties
    let properties = hashmap! {
        String::from("xenon.adaptors.schedulers.ssh.strictHostKeyChecking") => String::from("false")
    };

    // A SLURM scheduler requires the protocol scheme in the address.
    let location = if adaptor == *"slurm" {
        format!("ssh://{}", location)
    } else {
        location
    };

    // If it's a certificate, store the secret locally (// TODO: is this safe practice??)
    let credential = if let Credential::Certificate(CertificateCredential {
        username,
        certificate,
        passphrase,
    }) = credential
    {
        // // Decode the certificate
        // let certificate = match base64::decode(certificate.replace("\n", "")) {
        //     Ok(certificate) => certificate,
        //     Err(err)        => { return Err(JobError::XenonCertBase64Error{ location_id: location_id.to_string(), err }); }
        // };

        // Create a local filesystem on the endpoint
        let mut local = match FileSystem::create_local(xenon_endpoint.clone()).await {
            Ok(local) => local,
            Err(err)  => { return Err(JobError::XenonFilesystemError{ endpoint: xenon_endpoint, location_id: location_id.to_string(), err }); }
        };
        let certificate_file = format!("/keys/{}", get_random_identifier());

        // Write the certificate file
        let path = FileSystemPath::new(&certificate_file);
        if let Err(err) = local.write_to_file(certificate, &path).await { return Err(JobError::XenonFileWriteError{ filename: certificate_file, endpoint: xenon_endpoint, location_id: location_id.to_string(), err }); };

        // Return a new certificate that is a handle to this file
        Credential::new_certificate(certificate_file, username, passphrase)
    } else {
        credential
    };

    // Try to create the scheduler with the given credentials
    let scheduler = match Scheduler::create(adaptor.clone(), location, credential, xenon_endpoint.clone(), Some(properties)).await {
        Ok(scheduler) => scheduler,
        Err(err)      => { return Err(JobError::XenonSchedulerError{ adaptor, endpoint: xenon_endpoint, location_id: location_id.to_string(), err }); }
    };
    xenon_schedulers.insert(location_id.to_string(), Arc::new(RwLock::new(scheduler)));

    // Return a clone of the reference to the just-added scheduler
    let scheduler = xenon_schedulers.get(location_id).unwrap();
    let scheduler = scheduler.value().clone();
    Ok(scheduler)
}
/*******/

/* TIM */
/// **Edited: now not returning errors anymore.**
/// 
/// Creates a JobDescription for use with Docker.
/// 
/// **Arguments**
///  * `command`: The Command to create a job description of.
///  * `job_id`: The Job ID of the job to create a description for.
///  * `environment`: The environment variables for the job.
///  * `network`: The Docker network to connect the image to.
/// 
/// **Returns**  
/// The description of the job as a JobDescription object.
fn create_docker_job_description(
    command: &Command,
    job_id: &str,
    environment: HashMap<String, String>,
    network: Option<String>,
) -> JobDescription {
    let command = command.clone();

    // Format: docker run [-v /source:/target] {image} {arguments}
    let executable = String::from("docker");
    let mut arguments = vec![
        String::from("run"),
        String::from("--rm"),
        String::from("--name"),
        job_id.to_string(),
        String::from("--privileged"),
        // String::from("ALL"),
        // String::from("--cap-add"),
        // String::from("NET_ADMIN"),
        // String::from("--cap-add"),
        // String::from("NET_BIND_SERVICE"),
        // String::from("--cap-add"),
        // String::from("NET_RAW"),
    ];

    // if environment.contains_key(BRANE_MOUNT_DFS) {
    //     arguments.push(String::from("--cap-add"));
    //     arguments.push(String::from("SYS_ADMIN"));
    //     arguments.push(String::from("--device"));
    //     arguments.push(String::from("/dev/fuse"));
    //     arguments.push(String::from("--security-opt"));
    //     arguments.push(String::from("apparmor:unconfined"));
    // }

    arguments.push(String::from("--network"));
    if let Some(network) = network {
        arguments.push(network);
        arguments.push(String::from("--hostname"));
        arguments.push(job_id.to_string());
    } else {
        arguments.push(String::from("host"));
    }

    // Add environment variables
    for (name, value) in environment {
        arguments.push(String::from("--env"));
        arguments.push(format!("{}={}", name, value));
    }

    // Add mount bindings
    for mount in command.mounts {
        arguments.push(String::from("-v"));
        arguments.push(format!("{}:{}", mount.source, mount.destination));
    }

    // Extract the digest from the image, if any
    let image = command.image.expect("unreachable!");
    let image: &str = if image.contains('@') {
        &image[..image.find('@').unwrap()]
    } else {
        &image
    };

    // Add image
    arguments.push(image.to_string());

    // Add command
    arguments.push(String::from("--debug"));
    arguments.extend(command.command);

    debug!("[job {}] arguments: {}", job_id, arguments.join(" "));
    debug!("[job {}] executable: {}", job_id, executable);

    JobDescription {
        queue: Some(String::from("unlimited")),
        arguments: Some(arguments),
        executable: Some(executable),
        stdout: Some(format!("stdout-{}.txt", job_id)),
        stderr: Some(format!("stderr-{}.txt", job_id)),
        ..Default::default()
    }
}
/*******/

/* TIM */
/// **Edited: now not returning errors anymore.**
/// 
/// Creates a JobDescription for use with Singularity.
/// 
/// **Arguments**
///  * `command`: The Command to create a job description of.
///  * `job_id`: The Job ID of the job to create a description for.
///  * `environment`: The environment variables for the job.
/// 
/// **Returns**  
/// The description of the job as a JobDescription object.
fn create_singularity_job_description(
    command: &Command,
    job_id: &str,
    environment: HashMap<String, String>,
) -> JobDescription {
    let command = command.clone();

    // TODO: don't require sudo
    let executable = String::from("sudo");
    let mut arguments = vec![
        String::from("singularity"),
        String::from("run"),
        String::from("--nohttps"),
    ];

    if !environment.contains_key(BRANE_MOUNT_DFS) {
        arguments.push(String::from("--drop-caps"));
        arguments.push(String::from("ALL"));
        arguments.push(String::from("--add-caps"));
        arguments.push(String::from("CAP_NET_ADMIN,CAP_NET_BIND_SERVICE,CAP_NET_RAW"));
    }

    // Add environment variables
    for (name, value) in environment {
        arguments.push(String::from("--env"));
        arguments.push(format!("{}={}", name, value));
    }

    // Add mount bindings
    for mount in command.mounts {
        arguments.push(String::from("-B"));
        arguments.push(format!("{}:{}", mount.source, mount.destination));
    }

    // Extract the digest from the image, if any
    let image = command.image.expect("unreachable!");
    let image: &str = if image.contains('@') {
        &image[..image.find('@').unwrap()]
    } else {
        &image
    };

    // Add image
    arguments.push(format!("docker://{}", image));

    // Add command
    arguments.extend(command.command);

    JobDescription {
        arguments: Some(arguments),
        executable: Some(executable),
        stdout: Some(format!("stdout-{}.txt", job_id)),
        stderr: Some(format!("stderr-{}.txt", job_id)),
        ..Default::default()
    }
}
/*******/

///
///
///
fn get_random_identifier() -> String {
    let mut rng = rand::thread_rng();

    let identifier: String = iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(10)
        .collect();

    identifier.to_lowercase()
}
