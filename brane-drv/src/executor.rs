use crate::grpc;
use anyhow::Result;
use async_trait::async_trait;
use brane_bvm::executor::{VmExecutor, ExecutorError};
use brane_cfg::Infrastructure;
use brane_job::interface::{Command, CommandKind};
use brane_shr::jobs::JobStatus;
use bytes::BytesMut;
use dashmap::DashMap;
use prost::Message as _;
use rand::distributions::Alphanumeric;
use rand::{self, Rng};
use rdkafka::message::ToBytes;
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
};
use specifications::common::{FunctionExt, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::Sender;
use tonic::Status;
use uuid::Uuid;

///
///
///
#[derive(Clone)]
pub struct JobExecutor {
    pub client_tx: Sender<Result<grpc::ExecuteReply, Status>>,
    pub command_topic: String,
    pub producer: FutureProducer,
    pub session_uuid: String,
    pub states: Arc<DashMap<String, JobStatus>>,
    pub results: Arc<DashMap<String, Value>>,
    pub locations: Arc<DashMap<String, String>>,
    pub infra: Infrastructure,
}

impl JobExecutor {
    ///
    ///
    ///
    fn get_random_identifier(&self) -> String {
        let mut rng = rand::thread_rng();

        let identifier: String = std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(6)
            .collect();

        identifier.to_lowercase()
    }
}

#[async_trait]
impl VmExecutor for JobExecutor {
    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait. This also means we implemented proper error handling in this function.**
    ///
    /// Calls an external function on the given Brane infrastructure site.
    /// 
    /// **Arguments**  
    ///  * `function`: The function to execute remotely.
    ///  * `arguments`: A map of key/value pairs that are passed to the function to be executed.
    ///  * `location`: The location/site where the function will be executed.
    /// 
    /// **Returns**  
    /// The value of the external call if successful, or an ExecutorError otherwise.
    async fn call(
        &self,
        function: FunctionExt,
        arguments: HashMap<String, Value>,
        location: Option<String>,
    ) -> Result<Value, ExecutorError> {
        debug!("Processing external call for function '{}'...", function.name);
        let image = format!("{}:{}", function.package, function.version);
        debug!(" > associated image: {}...", image);
        let command = vec![
            function.kind.to_string(),
            function.name.to_string(),
            base64::encode(serde_json::to_string(&arguments).unwrap()),
        ];

        let session_uuid = Uuid::parse_str(&self.session_uuid).unwrap();
        let session_uuid_simple = session_uuid.to_simple().to_string();

        let random_id = self.get_random_identifier();
        let correlation_id = format!("A{}R{}", &session_uuid_simple[..8], random_id);

        let command = Command::new(
            CommandKind::Create,
            Some(correlation_id.clone()),
            Some(self.session_uuid.clone()),
            location,
            Some(image),
            command,
            None,
        );

        let mut payload = BytesMut::with_capacity(64);
        command.encode(&mut payload).unwrap();
        debug!("Sending command: \"{:?}\" (encoded: \"{:?}\").", command, payload);

        let message = FutureRecord::to(&self.command_topic)
            .key(&correlation_id)
            .payload(payload.to_bytes());

        let timeout = Timeout::After(Duration::from_secs(5));
        if self.producer.send(message, timeout).await.is_err() {
            // bail!("Failed to send command to '{}' topic.", self.command_topic);
            return Err(ExecutorError::UnsupportedError{ executor: "JobExecutor".to_string(), operation: "NOTHING".to_string() });
        }

        if function.detached {
            // Wait until "created" (address known ?)
            let created = WaitUntil {
                at_least: JobStatus::Created,
                correlation_id: correlation_id.clone(),
                states: self.states.clone(),
            };

            info!("Waiting until (detached) job '{}' is created...", correlation_id);
            created.await;
            info!("OK, job '{}' has been created", correlation_id);

            let location = self
                .locations
                .get(&correlation_id)
                .map(|s| s.clone())
                .unwrap_or_default();

            let location = self.infra.get_location_metadata(location).unwrap();

            let mut properties = HashMap::default();
            properties.insert(String::from("identifier"), Value::Unicode(correlation_id));
            properties.insert(String::from("address"), Value::Unicode(location.get_address()));

            Ok(Value::Struct {
                data_type: String::from("Service"),
                properties,
            })
        } else {
            /* TIM */
            // let finished = WaitUntil {
            //     at_least: JobStatus::Finished,
            //     correlation_id: correlation_id.clone(),
            //     states: self.states.clone(),
            // };

            // info!("Waiting until job '{}' is finished...", correlation_id);
            // finished.await;
            // info!("OK, job '{}' has been finished", correlation_id);

            // let (_, value) = self.results.remove(&correlation_id).unwrap();
            // let (_, value) = self.results.remove(&correlation_id).with_context(|| format!("Could not remove correlation ID '{}' from list", correlation_id))?;

            // Instead, call our own future
            let finished = WaitUntilFinished::new(correlation_id.clone(), self.states.clone());

            info!("Waiting until job '{}' is finished...", correlation_id);
            let status = finished.await;
            info!("OK, job '{}' has been finished", correlation_id);

            // Switch on the status
            return match status {
                JobStatus::Finished => {
                    // Get the value from the list
                    let (_, value) = self.results.remove(&correlation_id).unwrap_or((String::new(), Value::Unit));
                    // Remove the job from the list of statusses too
                    self.states.remove(&correlation_id);

                    // Return the result
                    debug!("RESULT: {:?}", value);
                    Ok(value)
                }

                JobStatus::Stopped => {
                    // Return that the job was forcefully stopped
                    debug!("RESULT: <Job stopped>");
                    if let Err(_) = self.stderr(String::from("Job stopped prematurely")).await { /* Do nothing */ }
                    // Err(anyhow!("Job was stopped prematurely."))
                    Err(ExecutorError::UnsupportedError{ executor: "JobExecutor".to_string(), operation: "NOTHING".to_string() })
                }

                JobStatus::Failed => {
                    debug!("RESULT: <Job failed>");
                    if let Err(_) = self.stderr(format!("Job failed to run: {}", "???")).await { /* Do nothing */ }
                    // Err(anyhow!("Job failed to run: {}", "???"))
                    Err(ExecutorError::UnsupportedError{ executor: "JobExecutor".to_string(), operation: "NOTHING".to_string() })
                }

                _ => {
                    panic!("Encountered JobStatus {:?} after WaitUntilFinished, which should never happen!", status);
                }
            }

            /*******/
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Sends a message to the client debug channel.
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
        let reply = grpc::ExecuteReply {
            close: false,
            debug: Some(text),
            stderr: None,
            stdout: None,
        };

        if let Err(reason) = self.client_tx.send(Ok(reply)).await {
            return Err(ExecutorError::ClientTxError{ err: format!("{}", reason) });
        }
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Sends a message to the client stderr.
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
        let reply = grpc::ExecuteReply {
            close: false,
            debug: None,
            stderr: Some(text),
            stdout: None,
        };

        if let Err(reason) = self.client_tx.send(Ok(reply)).await {
            return Err(ExecutorError::ClientTxError{ err: format!("{}", reason) });
        }
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Sends a message to the client stdout.
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
        let reply = grpc::ExecuteReply {
            close: false,
            debug: None,
            stderr: None,
            stdout: Some(text),
        };

        if let Err(reason) = self.client_tx.send(Ok(reply)).await {
            return Err(ExecutorError::ClientTxError{ err: format!("{}", reason) });
        }
        Ok(())
    }
    /*******/

    /* TIM */
    // TODO????
    /// **Edited: Synced Call up with the VmExecutor trait.**
    ///
    /// Launches a new job and waits until it has reached the target ServiceState.
    /// 
    /// **Arguments**  
    ///  * `text`: The message to send.
    /// 
    /// **Returns**  
    /// Nothing if successfull, or an ExecutorError otherwise.
    async fn wait_until(
        &self,
        _service: String,
        _state: brane_bvm::executor::ServiceState,
    ) -> Result<(), ExecutorError> {
        Ok(())
    }
    /*******/
}

///
///
///
struct Wait {
    correlation_id: String,
    states: Arc<DashMap<String, JobStatus>>,
    results: Arc<DashMap<String, Value>>,
}

impl Future for Wait {
    type Output = Value;

    ///
    ///
    ///
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let state = self.states.get(&self.correlation_id);
        if let Some(state) = state {
            let state = state.value();
            match state {
                JobStatus::Failed => {
                    unimplemented!();
                }
                JobStatus::Finished => {
                    let (_, value) = self.results.remove(&self.correlation_id).unwrap();
                    self.states.remove(&self.correlation_id);

                    debug!("Job finished! Returning result");
                    return Poll::Ready(value);
                }
                JobStatus::Stopped => {
                    unimplemented!();
                }
                _ => {}
            }
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

///
///
///
struct WaitUntil {
    at_least: JobStatus,
    correlation_id: String,
    states: Arc<DashMap<String, JobStatus>>,
}

impl Future for WaitUntil {
    type Output = ();

    ///
    ///
    ///
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let state = self.states.get(&self.correlation_id);
        if let Some(state) = state {
            let state = state.value();

            if state >= &self.at_least {
                return Poll::Ready(());
            }
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/* TIM */
/// Re-coded Future of WaitUntil, that also takes into account more special job statusses like 'Failed' and 'Stopped'.
/// Looks a lot like Wait, but not sure how that's used so didn't wanna touch that
struct WaitUntilFinished {
    /// The correlation ID of the job we'd like to wait for
    correlation_id   : String,
    /// The list of states we saw coming by in the event manages
    states           : Arc<DashMap<String, JobStatus>>,
    // /// The list of responses we collected from our child jobs
    // responses      : Arc<DashMap<String, Value>>,
    // /// The last time we checked the status of the job
    // last_status_poll : std::time::SystemTime,
}

impl WaitUntilFinished {
    /// Constructor for WaitUntilFinished
    /// 
    /// **Arguments**
    ///  * `correlation_id`: The correlation ID of the job we'd like to wait for
    ///  * `states`: The list of states that contains up-to-date information on all job's states
    pub fn new(correlation_id: String, states: Arc<DashMap<String, JobStatus>>) -> WaitUntilFinished {
        return WaitUntilFinished {
            correlation_id,
            states,
            // last_status_poll: std::time::SystemTime::now()
        };
    }
}

impl Future for WaitUntilFinished {
    /// The return type for the Future job. For us, this is its status.
    type Output = JobStatus;

    /// Polls the Future whether it is ready or not, 'performing work' in the process
    /// Note that it should never block.
    /// 
    /// **Arguments**
    ///  * `cx`: Context for the job we're processing.
    ///
    /// **Returns**  
    /// The state of the Job now that it's done.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // See if the job has undergone status changes
        let state = self.states.get(&self.correlation_id);
        if let Some(state) = state {
            let state = state.value();

            // Switch on the state
            match state {
                JobStatus::Finished |
                JobStatus::Failed |
                JobStatus::Stopped => { return Poll::Ready(*state); }
                _ => {}
            }
        }

        // The job didn't finish, so check again in DEFAULT_POLL_TIMEOUT ms
        // TODO the timeout part
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
/*******/
