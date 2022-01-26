use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use specifications::common::{FunctionExt, Value};

/* TIM */
/// Public enum representing various errors for the Executor
#[derive(Debug)]
pub enum ExecutorError {
    /// Error for when an operation isn't supported in this executor
    UnsupportedError{ executor: String, operation: String },

    /// Could not send a message to the client
    ClientTxError{ err: String },
}

impl std::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutorError::UnsupportedError{ executor, operation } => write!(f, "Executor '{}' doesn't support {}", executor, operation),

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

#[derive(Clone)]
pub struct NoExtExecutor {}

impl Default for NoExtExecutor {
    fn default() -> Self {
        Self {}
    }
}

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
