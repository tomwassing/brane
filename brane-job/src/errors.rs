/* ERRORS.rs
 *   by Lut99
 *
 * Created:
 *   07 Feb 2022, 10:20:50
 * Last edited:
 *   21 Mar 2022, 21:33:55
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains the errors that are used in the brane-job package.
**/

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::path::PathBuf;

use brane_cfg::infrastructure::{LocationCredentials, InfrastructureError};
use prost::{EncodeError, DecodeError};
use rdkafka::error::{KafkaError, RDKafkaErrorCode};


/***** ERRORS *****/
/// Lists the top-most errors in the brane-job service.
#[derive(Debug)]
pub enum JobError {
    /// Could not create a Kafka client
    KafkaClientError{ servers: String, err: KafkaError },
    /// Could not get the Kafka client to try to add more topics
    KafkaTopicsError{ topics: String, err: KafkaError },
    /// Could not add the given topic (with a duplicate error already filtered out)
    KafkaTopicError{ topic: String, err: RDKafkaErrorCode },
    /// Could not create a Kafka producer
    KafkaProducerError{ servers: String, err: KafkaError },
    /// Could not create a Kafka consumer
    KafkaConsumerError{ servers: String, id: String, err: KafkaError },

    /// Could not get the Kafka commit offsets
    KafkaGetOffsetError{ clb: String, cmd: String, err: KafkaError },
    /// Could not update the Kafka commit offsets
    KafkaSetOffsetError{ topic: String, kind: String, err: KafkaError },
    /// Could not commit the update to the Kafka commit offsets
    KafkaSetOffsetsError{ clb: String, cmd: String, err: KafkaError },

    /// Could not encode an event for sending
    EventEncodeError{ key: String, err: EncodeError },
    /// Could not decode a message into a Callback struct
    CallbackDecodeError{ key: String, err: DecodeError },
    /// Could not decode a message into a Command struct
    CommandDecodeError{ key: String, err: DecodeError },
    /// Given integer is not a valid CallbackKind
    IllegalCallbackKind{ kind: i32 },
    /// Given integer is not a valid CommandKind
    IllegalCommandKind{ kind: i32 },

    /// A given Command struct has a field not set
    IllegalCommandError{ key: String, kind: String, field: String },

    /// Illegal credential type for a Kubernetes cluster
    K8sIllegalCredentials{ location_id: String, cred_type: String },
    /// A given Kubernetes configuration file cannot be decoded as base64
    K8sBase64Error{ location_id: String, err: base64::DecodeError },
    /// A given Kubernetes configuration file cannot be decoded as UTF-8
    K8sUTF8Error{ location_id: String, err: std::string::FromUtf8Error },
    /// A given Kubernetes configuration file cannot be parsed as YAML
    K8sYAMLError{ location_id: String, err: serde_yaml::Error },
    /// A given Kubernetes configuration file cannot be parsed as an actual configuration file
    K8sConfigError{ location_id: String, err: kube::Error },
    /// Could not construct a client from the given configuration file
    K8sClientError{ location_id: String, err: kube::Error },
    /// Could not create the JobDescription from the internal JSON file
    K8sJobDescriptionError{ job_id: String, location_id: String, err: serde_json::Error },
    /// Could not create the missing Kubernetes namespace
    K8sNamespaceError{ location_id: String, namespace: String, err: serde_json::Error },
    /// Could not launch a Kubernetes job
    K8sCreateJobError{ job_id: String, location_id: String, err: kube::Error },

    /// The given image file could not be read
    ImageReadError{ path: PathBuf, err: tokio::io::Error },
    /// Could not connect to the local Docker instance
    DockerConnectionFailed{ err: bollard::errors::Error },
    /// Could not import the image at the given path
    DockerImportError{ path: PathBuf, err: bollard::errors::Error },
    /// Could not create the given image
    DockerCreateImageError{ image: String, err: bollard::errors::Error },
    /// Could not create the given container from the given image
    DockerCreateContainerError{ name: String, image: String, err: bollard::errors::Error },
    /// Could not start the given container from the given image
    DockerStartError{ name: String, image: String, err: bollard::errors::Error },
    /// Could not wait for container to complete
    DockerWaitError{ name: String, image: String, err: bollard::errors::Error },
    /// Could not get logs from the given container
    DockerLogsError{ name: String, image: String, err: bollard::errors::Error },
    /// Could not inspect the given container
    DockerInspectContainerError{ name: String, err: bollard::errors::Error },
    /// Could not remove the given container
    DockerRemoveContainerError{ name: String, err: bollard::errors::Error },
    /// Could not remove the given image
    DockerRemoveImageError{ name: String, id: String, err: bollard::errors::Error },

    /// A Docker container had no runningstate once it was finished
    DockerContainerNoState{ name: String },
    /// A Docker container had no exit code once it was finished
    DockerContainerNoExitCode{ name: String },
    /// A container did not have a network while we expected one
    DockerContainerNoNetwork{ name: String },

    /// Illegal credential type for a Slurm scheduler
    SlurmIllegalCredentials{ location_id: String, cred_type: String },

    /// Could not check if the given xenon scheduler is still open for writing
    XenonIsOpenError{ location_id: String, err: anyhow::Error },
    /// Could not decode a certificate as Base64
    XenonCertBase64Error{ location_id: String, err: base64::DecodeError },
    /// Could not create a local filesystem on the Xenon endpoint
    XenonFilesystemError{ endpoint: String, location_id: String, err: anyhow::Error },
    /// Could not create/write a file on the filesystem of a Xenon endpoint
    XenonFileWriteError{ filename: String, endpoint: String, location_id: String, err: anyhow::Error },
    /// Could not create a Xenon scheduler
    XenonSchedulerError{ adaptor: String, endpoint: String, location_id: String, err: anyhow::Error },
    /// The given runtime is not applicable
    XenonUnknownRuntime{ runtime: String, location_id: String },
    /// Could not submit a Xenon job
    XenonSubmitError{ job_id: String, adaptor: String, location_id: String, err: anyhow::Error },

    /// Could not properly get information from the infrastructure file
    InfrastructureError{ err: InfrastructureError },
}

impl JobError {
    /// Serializes a given list of vectors into a string.
    /// 
    /// **Generic types**
    ///  * `T`: The type of the vector. Must be convertible to string via the Display trait.
    /// 
    /// **Arguments**
    ///  * `v`: The Vec to serialize.
    /// 
    /// **Returns**  
    /// A string describing the vector. Nothing too fancy, just a list separated by commas.
    pub fn serialize_vec<T>(v: &[T]) -> String
    where
        T: Display
    {
        let mut res: String = String::new();
        for e in v {
            if res.is_empty() { res += ", "; }
            res += &format!("'{}'", e);
        }
        res
    }
}

impl Display for JobError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            JobError::KafkaClientError{ servers, err }       => write!(f, "Could not create Kafka client with bootstrap servers '{}': {}", servers, err),
            JobError::KafkaTopicsError{ topics, err }        => write!(f, "Could not create new Kafka topics '{}': {}", topics, err),
            JobError::KafkaTopicError{ topic, err }          => write!(f, "Coult not create Kafka topic '{}': {}", topic, err),
            JobError::KafkaProducerError{ servers, err }     => write!(f, "Could not create Kafka producer with bootstrap servers '{}': {}", servers, err),
            JobError::KafkaConsumerError{ servers, id, err } => write!(f, "Could not create Kafka consumer for ID '{}' with bootstrap servers '{}': {}", id, servers, err),

            JobError::KafkaGetOffsetError{ clb, cmd, err }    => write!(f, "Could not get offsets for topics '{}' (callback) and '{}' (command): {}", clb, cmd, err),
            JobError::KafkaSetOffsetError{ topic, kind, err } => write!(f, "Could not set offsets for topic '{}' ({}): {}", topic, kind, err),
            JobError::KafkaSetOffsetsError{ clb, cmd, err }   => write!(f, "Could not commit offsets for topics '{}' (callback) and '{}' (command): {}", clb, cmd, err),

            JobError::EventEncodeError{ key, err }    => write!(f, "Could not encode event message (key: {}) for sending: {}", key, err),
            JobError::CallbackDecodeError{ key, err } => write!(f, "Could not decode message (key: {}) as a callback message: {}", key, err),
            JobError::CommandDecodeError{ key, err }  => write!(f, "Could not decode message (key: {}) as a command message: {}", key, err),
            JobError::IllegalCallbackKind{ kind }     => write!(f, "Unknown callback kind '{}'", kind),
            JobError::IllegalCommandKind{ kind }      => write!(f, "Unknown command kind '{}'", kind),

            JobError::IllegalCommandError{ key, kind, field } => write!(f, "Incoming {} command message (key: {}) has field '{}' unset", kind, key, field),

            JobError::K8sIllegalCredentials{ location_id, cred_type }    => write!(f, "Cannot use {} credentials for Kubernetes site '{}': expected {}", cred_type, location_id, LocationCredentials::Config{ file: String::new() }.cred_type()),
            JobError::K8sBase64Error{ location_id, err }                 => write!(f, "Cannot decode Kubernetes config file for site '{}' as Base64: {}", location_id, err),
            JobError::K8sUTF8Error{ location_id, err }                   => write!(f, "Cannot decode Kubernetes config file for site '{}' as UTF-8: {}", location_id, err),
            JobError::K8sYAMLError{ location_id, err }                   => write!(f, "Cannot parse Kubernetes config file for site '{}' as YAML: {}", location_id, err),
            JobError::K8sConfigError{ location_id, err }                 => write!(f, "Cannot parse Kubernetes config file for site '{}': {}", location_id, err),
            JobError::K8sClientError{ location_id, err }                 => write!(f, "Cannot create client from the Kubernetes config file of site '{}': {}", location_id, err),
            JobError::K8sJobDescriptionError{ job_id, location_id, err } => write!(f, "Creating job description for job '{}' on site '{}' failed: {}", job_id, location_id, err),
            JobError::K8sNamespaceError{ location_id, namespace, err }   => write!(f, "Creating namespace '{}' on site '{}' failed: {}", namespace, location_id, err),
            JobError::K8sCreateJobError{ job_id, location_id, err }      => write!(f, "Could not create job '{}' on site '{}': {}", job_id, location_id, err),

            JobError::ImageReadError{ path, err }                    => write!(f, "Cannot read image '{}' for import: {}", path.display(), err),
            JobError::DockerConnectionFailed{ err }                  => write!(f, "Could not connect to local Docker instance: {}", err),
            JobError::DockerImportError{ path, err }                 => write!(f, "Cannot import Docker image '{}': {}", path.display(), err),
            JobError::DockerCreateImageError{ image, err }           => write!(f, "Cannot create Docker image '{}': {}", image, err),
            JobError::DockerCreateContainerError{ name, image, err } => write!(f, "Could not create Docker container '{}' from image '{}': {}", name, image, err),
            JobError::DockerStartError{ name, image, err }           => write!(f, "Could not start Docker container '{}' from image '{}': {}", name, image, err),
            JobError::DockerWaitError{ name, image, err }            => write!(f, "Could not wait for Docker container '{}' (from image '{}') to complete: {}", name, image, err),
            JobError::DockerLogsError{ name, image, err }            => write!(f, "Could not retrieve logs from Docker container '{}' (from image '{}'): {}", name, image, err),
            JobError::DockerInspectContainerError{ name, err }       => write!(f, "Could not inspect Docker container '{}': {}", name, err),
            JobError::DockerRemoveContainerError{ name, err }        => write!(f, "Could not remove Docker container '{}': {}", name, err),
            JobError::DockerRemoveImageError{ name, id, err }        => write!(f, "Could not remove Docker image '{}' (id: {}): {}", name, id, err),

            JobError::DockerContainerNoState{ name }    => write!(f, "Docker container '{}' has no state after running", name),
            JobError::DockerContainerNoExitCode{ name } => write!(f, "Docker container '{}' has no exit code after running", name),
            JobError::DockerContainerNoNetwork{ name }  => write!(f, "Docker container '{}' has no networks: expected at least 1", name),

            JobError::SlurmIllegalCredentials{ location_id, cred_type } => write!(f, "Cannot use {} credentials for Slurm site '{}': expected {} or {}", cred_type, location_id, LocationCredentials::SshCertificate{ username: String::new(), certificate: String::new(), passphrase: None }.cred_type(), LocationCredentials::SshPassword{ username: String::new(), password: String::new() }.cred_type()),

            JobError::XenonIsOpenError{ location_id, err }                        => write!(f, "Cannot check if the Xenon scheduler for site '{}' is open: {}", location_id, err),
            JobError::XenonCertBase64Error{ location_id, err }                    => write!(f, "Could not decode the certificate for site '{}' as Base64: {}", location_id, err),
            JobError::XenonFilesystemError{ endpoint, location_id, err }          => write!(f, "Could not create a local filesystem on Xenon endpoint '{}' for site '{}': {}", endpoint, location_id, err),
            JobError::XenonFileWriteError{ filename, endpoint, location_id, err } => write!(f, "Could not write local file '{}' on Xenon endpoint '{}' for site '{}': {}", filename, endpoint, location_id, err),
            JobError::XenonSchedulerError{ adaptor, endpoint, location_id, err }  => write!(f, "Could not create a Xenon scheduler with {} adaptor on endpoint '{}' for site '{}': {}", adaptor, endpoint, location_id, err),
            JobError::XenonUnknownRuntime{ runtime, location_id }                 => write!(f, "Unknown runtime '{}' for site '{}'; expected 'docker' or 'singularity'", runtime, location_id),
            JobError::XenonSubmitError{ job_id, adaptor, location_id, err }       => write!(f, "Could not submit job '{}' on a Xenon scheduler with {} adaptor on site '{}': {}", job_id, adaptor, location_id, err),

            JobError::InfrastructureError{ err } => write!(f, "Could not read infrastructure data: {}", err),
        }
    }
}

impl Error for JobError {}
