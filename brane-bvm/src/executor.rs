use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use specifications::common::{FunctionExt, Value};
use specifications::errors::EncodeDecodeError;
use specifications::package::PackageInfoError;
use specifications::version::Version;


/* TIM */
/// Public enum representing various errors for the Executor
#[derive(Debug)]
pub enum ExecutorError {
    /// Error for when an operation isn't supported in this executor
    UnsupportedError{ executor: String, operation: String },

    /// Given HashMap of arguments is invalid for some reason
    IllegalArguments{ args: HashMap<String, Value>, err: serde_json::Error },
    /// The given data directory could not be resolved exist
    IllegalDataDir{ path: PathBuf, err: std::io::Error },
    /// The given data directory does not exist
    DataDirDoesntExist{ path: PathBuf },
    /// The given data directory path could not be converted to a string for some reason
    UnreadableDataDir{ path: PathBuf },
    /// The given data directory contains a colon
    IllegalDataDirColon{ path: PathBuf },
    /// Could not get the directory of a package
    PackageDirError{ package: String, err: String },
    /// Could not read a PackageInfo for some reason
    PackageInfoError{ package: String, path: PathBuf, err: PackageInfoError },

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

    /// The external job failed to be created / started / w/e
    ExternalCallError{ name: String, package: String, version: Version, err: String },
    /// The external job failed, returning a non-zero exit code
    ExternalCallFailed{ name: String, package: String, version: Version, code: i32, stdout: String, stderr: String },
    /// The output of the external job could not be decoded properly.
    OutputDecodeError{ name: String, package: String, version: Version, stdout: String, err: EncodeDecodeError },

    /// Could not send a message to the client
    ClientTxError{ err: String },
}

impl std::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutorError::UnsupportedError{ executor, operation } => write!(f, "Executor '{}' doesn't support {}", executor, operation),

            ExecutorError::IllegalArguments{ args, err }          => write!(f, "Could not serialize arguments: {}\nArguments that failed to serialize:\n{:?}\n", err, args),
            ExecutorError::IllegalDataDir{ path, err }            => write!(f, "Given data directory '{}' could not be resolved: {}", path.display(), err),
            ExecutorError::DataDirDoesntExist{ path }             => write!(f, "Given data directory '{}' does not exist", path.display()),
            ExecutorError::UnreadableDataDir{ path }              => write!(f, "Given data directory '{}' could not be converted to a string", path.display()),
            ExecutorError::IllegalDataDirColon{ path }            => write!(f, "Given data directory '{}' contains illegal colon (':')", path.display()),
            ExecutorError::PackageDirError{ package, err }        => write!(f, "Cannot get package directory for package '{}': {}", package, err),
            ExecutorError::PackageInfoError{ package, path, err } => write!(f, "Cannot read PackageInfo file '{}' for package '{}': {}", path.display(), package, err),

            ExecutorError::ImageReadError{ path, err }                    => write!(f, "Cannot read image '{}' for import: {}", path.display(), err),
            ExecutorError::DockerConnectionFailed{ err }                  => write!(f, "Could not connect to local Docker instance: {}", err),
            ExecutorError::DockerImportError{ path, err }                 => write!(f, "Cannot import Docker image '{}': {}", path.display(), err),
            ExecutorError::DockerCreateImageError{ image, err }           => write!(f, "Cannot create Docker image '{}': {}", image, err),
            ExecutorError::DockerCreateContainerError{ name, image, err } => write!(f, "Could not create Docker container '{}' from image '{}': {}", name, image, err),
            ExecutorError::DockerStartError{ name, image, err }           => write!(f, "Could not start Docker container '{}' from image '{}': {}", name, image, err),
            ExecutorError::DockerWaitError{ name, image, err }            => write!(f, "Could not wait for Docker container '{}' (from image '{}') to complete: {}", name, image, err),
            ExecutorError::DockerLogsError{ name, image, err }            => write!(f, "Could not retrieve logs from Docker container '{}' (from image '{}'): {}", name, image, err),
            ExecutorError::DockerInspectContainerError{ name, err }       => write!(f, "Could not inspect Docker container '{}': {}", name, err),
            ExecutorError::DockerRemoveContainerError{ name, err }        => write!(f, "Could not remove Docker container '{}': {}", name, err),
            ExecutorError::DockerRemoveImageError{ name, id, err }        => write!(f, "Could not remove Docker image '{}' (id: {}): {}", name, id, err),

            ExecutorError::DockerContainerNoState{ name }    => write!(f, "Docker container '{}' has no state after running", name),
            ExecutorError::DockerContainerNoExitCode{ name } => write!(f, "Docker container '{}' has no exit code after running", name),
            ExecutorError::DockerContainerNoNetwork{ name }               => write!(f, "Docker container '{}' has no networks: expected at least 1", name),

            ExecutorError::ExternalCallError{ name, package, version, err }                   => write!(f, "External call to function '{}' from package '{}' (version {}) failed to launch:\n{}", name, package, version, err),
            ExecutorError::ExternalCallFailed{ name, package, version, code, stdout, stderr } => write!(f, "External call to function '{}' from package '{}' (version {}) failed with exit code {}:\n\nstdout:\n-------------------------------------------------------------------------------\n{}\n-------------------------------------------------------------------------------\n\nstderr:\n-------------------------------------------------------------------------------\n{}-------------------------------------------------------------------------------\n\n", name, package, version, code, stdout, stderr),
            ExecutorError::OutputDecodeError{ name, package, version, stdout, err }           => write!(f, "Could not decode output of function '{}' from package {} (version {}) from Base64: {}\n\nstdout:\n-------------------------------------------------------------------------------\n{}\n-------------------------------------------------------------------------------\n\n", name, package, version, err, stdout),

            ExecutorError::ClientTxError{ err } => write!(f, "Could not write message to remote client: {}", err),
        }
    }
}

impl std::error::Error for ExecutorError {}
/*******/

#[repr(u8)]
pub enum ServiceState {
    Created = 1,
    Started = 2,
    Done = 3,
}

#[async_trait]
pub trait VmExecutor {
    /* TIM */
    /// **Edited: changed return type of the call() function to include ExecutorErrors.**
    ///
    /// Calls an external function according to the actual Executor implementation.
    /// 
    /// **Arguments**
    ///  * `call`: The external function call to perform
    ///  * `arguments`: Arguments for the function as key/value pairs
    ///  * `location`: The location where the function should be run. Is a high-level location, defined in infra.yml.
    /// 
    /// **Returns**  
    /// The call's return Value on success, or an ExecutorError upon failure.
    async fn call(
        &self,
        call: FunctionExt,
        arguments: HashMap<String, Value>,
        location: Option<String>,
    ) -> Result<Value, ExecutorError>;
    /*******/

    /* TIM */
    /// **Edited: changed return type to also return ExecutorErrors.**
    /// 
    /// Writes a debug message to the client TX stream.
    ///
    /// **Arguments**
    ///  * `text`: The text to write.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or an ExecutorError otherwise.
    async fn debug(
        &self,
        text: String,
    ) -> Result<(), ExecutorError>;
    /*******/

    /* TIM */
    /// **Edited: changed return type to also return ExecutorErrors.**
    /// 
    /// Writes an error message to the client TX stream.
    ///
    /// **Arguments**
    ///  * `text`: The text to write.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or an ExecutorError otherwise.
    async fn stderr(
        &self,
        text: String,
    ) -> Result<(), ExecutorError>;
    /*******/

    /* TIM */
    /// **Edited: changed return type to also return ExecutorErrors.**
    /// 
    /// Writes a standard/info message to the client TX stream.
    ///
    /// **Arguments**
    ///  * `text`: The text to write.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or an ExecutorError otherwise.
    async fn stdout(
        &self,
        text: String,
    ) -> Result<(), ExecutorError>;
    /*******/

    /* TIM */
    /// **Edited: changed return type to also return ExecutorErrors.**
    ///
    /// Performs an external function call, but blocks until the call has reached the desired state instead of until it is completed.
    /// 
    /// **Arguments**
    ///  * `service`: The service to call.
    ///  * `state`: The state to wait for.
    /// 
    /// **Result**  
    /// Returns nothing if the service was launched successfully and the state reached, or an ExecutorError otherwise.
    async fn wait_until(
        &self,
        service: String,
        state: ServiceState,
    ) -> Result<(), ExecutorError>;
    /*******/
}

#[derive(Clone, Default)]
pub struct NoExtExecutor {}

#[async_trait]
impl VmExecutor for NoExtExecutor {
    /* TIM */
    /// **Edited: matched function signature to that of the VmExecutor trait.**
    ///
    /// Doesn't call anything, just throws an UnsupportedError from the ExecutorError enum.
    async fn call(
        &self,
        _: FunctionExt,
        _: HashMap<String, Value>,
        _: Option<String>,
    ) -> Result<Value, ExecutorError> {
        Err(ExecutorError::UnsupportedError{ executor: String::from("NoExtExecutor"), operation: String::from("external function calls") })
    }
    /*******/

    /* TIM */
    /// **Edited: matched function signature to that of the VmExecutor trait.**
    ///
    /// Simply writes the message using the logger's debug! macro
    async fn debug(
        &self,
        text: String,
    ) -> Result<(), ExecutorError> {
        debug!("{}", text);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: matched function signature to that of the VmExecutor trait.**
    ///
    /// Simply writes the message using the standard eprintln! macro
    async fn stderr(
        &self,
        text: String,
    ) -> Result<(), ExecutorError> {
        eprintln!("{}", text);
        Ok(())
    }

    /* TIM */
    /// **Edited: matched function signature to that of the VmExecutor trait.**
    ///
    /// Simply writes the message using the standard println! macro
    async fn stdout(
        &self,
        text: String,
    ) -> Result<(), ExecutorError> {
        println!("{}", text);
        Ok(())
    }

    /* TIM */
    /// **Edited: matched function signature to that of the VmExecutor trait.**
    ///
    /// Doesn't call anything, just returns the UnsupportedError from the ExecutorError enum.
    async fn wait_until(
        &self,
        _: String,
        _: ServiceState,
    ) -> Result<(), ExecutorError> {
        Err(ExecutorError::UnsupportedError{ executor: String::from("NoExtExecutor"), operation: String::from("external function calls") })
    }
}
