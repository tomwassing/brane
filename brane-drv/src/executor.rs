use crate::grpc;
use anyhow::Result;
use async_trait::async_trait;
use brane_bvm::executor::{VmExecutor, ExecutorError};
use brane_cfg::Infrastructure;
use brane_job::interface::{Command, CommandKind, FailureResult};
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
use std::time::SystemTime;
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::Sender;
use tonic::Status;
use uuid::Uuid;


/***** CONSTANTS *****/
/// Determines the timeout (in milliseconds) we give the job until we expect to hear something about its created status (Created or CreateFailed)
const DEFAULT_CREATED_TIMEOUT     : u128 = 60 * 1000;
/// Determines the timeout (in milliseconds) we give the job until we expect its first event (Ready)
const DEFAULT_READY_TIMEOUT       : u128 = 60 * 1000;
/// Determines the timeout (in milliseconds) we give the job until we expect it to initiaize its directories (Initialized or InitializeFailed)
const DEFAULT_INITIALIZED_TIMEOUT : u128 = 30 * 1000;
/// Determines the timeout (in milliseconds) we give the job until we expect it to actually start running (Started or StartFailed)
const DEFAULT_STARTED_TIMEOUT     : u128 = 10 * 1000;
/// Determines the timeout (in milliseconds) we want at most in between heartbeats for a job
const DEFAULT_HEARTBEAT_TIMEOUT   : u128 = 10 * 1000;
/// Determines the timeout (in milliseconds) we give the job between completing and returning a result
const DEFAULT_RESULT_TIMEOUT      : u128 = 30 * 1000;





/***** ERRORS *****/
/// Describes errors that can occur when scheduling jobs
#[derive(Debug)]
enum ScheduleError {
    /// The Job node did not report 'created' or 'created failed' within time
    JobCreatedTimeout{ correlation_id: String },
    /// The Job node returned a CreateFailed event
    JobCreateFailed{ correlation_id: String, err: String },

    /// The Job with the given correlation ID failed to emit a 'Ready' within the timeout
    JobReadyTimeout{ correlation_id: String },
    /// The Job with the given correlation ID failed to emit an 'Initialized' within the timeout
    JobInitializedTimeout{ correlation_id: String },
    /// The Job node returned an InitializeFailed event
    JobInitializeFailed{ correlation_id: String, err: String },
    /// The Job with the given correlation ID failed to emit a 'Started' within the timeout
    JobStartedTimeout{ correlation_id: String },
    /// The Job node returned a StartFailed event
    JobStartFailed{ correlation_id: String, err: String },
    /// The Job with the given correlation ID failed to emit a 'Heartbeat' within the timeout
    JobHeartbeatTimeout{ correlation_id: String },
    /// The Job node returned a CompleteFailed event
    JobCompleteFailed{ correlation_id: String, err: String },

    /// The job didn't respond stopped, failed or finished in time
    JobResultTimeout{ correlation_id: String },
    /// Could not decode the output of the job
    JobDecodeFailed{ correlation_id: String, err: String },
    /// The job was stopped
    JobStopped{ correlation_id: String, signal: String },
    /// The job failed by itself
    JobFailed{ correlation_id: String, code: i32, stdout: String, stderr: String },

    /// Could not deserialize the output from a failed job
    FailedDeserializeError{ output: String, err: serde_json::Error },
    /// Could not deserialize the output from a finished job
    FinishedDeserializeError{ output: String, err: serde_json::Error },
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleError::JobCreatedTimeout{ correlation_id }    => write!(f, "Job node failed to create job '{}' within {} seconds (is the Job node online?)", correlation_id, DEFAULT_CREATED_TIMEOUT / 1000),
            ScheduleError::JobCreateFailed{ correlation_id, err } => write!(f, "Could not create job '{}': {}", correlation_id, err),

            ScheduleError::JobReadyTimeout{ correlation_id }          => write!(f, "Job '{}' failed to report alive within {} seconds", correlation_id, DEFAULT_READY_TIMEOUT / 1000),
            ScheduleError::JobInitializedTimeout{ correlation_id }    => write!(f, "Job '{}' failed to prepare running within {} seconds", correlation_id, DEFAULT_INITIALIZED_TIMEOUT / 1000),
            ScheduleError::JobInitializeFailed{ correlation_id, err } => write!(f, "Could not initialize job '{}': {}", correlation_id, err),
            ScheduleError::JobStartedTimeout{ correlation_id }        => write!(f, "Job '{}' failed to start running within {} seconds", correlation_id, DEFAULT_STARTED_TIMEOUT / 1000),
            ScheduleError::JobStartFailed{ correlation_id, err }      => write!(f, "Could not start job '{}': {}", correlation_id, err),
            ScheduleError::JobHeartbeatTimeout{ correlation_id }      => write!(f, "Job '{}' didn't send a heartbeat for {} seconds; considering it dead", correlation_id, DEFAULT_HEARTBEAT_TIMEOUT / 1000),
            ScheduleError::JobCompleteFailed{ correlation_id, err }   => write!(f, "Could not complete job '{}': {}", correlation_id, err),

            ScheduleError::JobResultTimeout{ correlation_id }                => write!(f, "Job '{}' didn't send result within {} seconds", correlation_id, DEFAULT_RESULT_TIMEOUT / 1000),
            ScheduleError::JobDecodeFailed{ correlation_id, err }            => write!(f, "Could not decode output of job '{}': {}", correlation_id, err),
            ScheduleError::JobStopped{ correlation_id, signal }              => write!(f, "Job '{}' failed because it was stopped externally (signal {})", correlation_id, signal),
            ScheduleError::JobFailed{ correlation_id, code, stdout, stderr } => {
                let separator = (0..80).map(|_| '-').collect::<String>();
                write!(f, "Job '{}' failed by returning non-zero exit code {}:\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n\n", correlation_id, code, separator, stdout, separator, separator, stderr, separator)
            },

            ScheduleError::FailedDeserializeError{ output, err }   => write!(f, "Could not deserialize '{}' as a valid code/stdout/stderr triplet: {}", output, err),
            ScheduleError::FinishedDeserializeError{ output, err } => write!(f, "Could not deserialize '{}' as a valid Value: {}", output, err),
        }
    }
}

impl std::error::Error for ScheduleError {}





/***** FUTURES *****/
/// Waits until the given job reaches Completed before it timeouts by missing heartbeats
struct WaitUntilNewState {
    /// The correlation ID of the job we're waiting for
    correlation_id : String,
    /// The current state, which we use to check if a new state arrived
    current_state  : JobStatus,

    /// The event-monitor updated list of last heartbeat times we use to check the job's alive status. If None, then not accepting heartbeats.
    heartbeats     : Option<Arc<DashMap<String, SystemTime>>>,
    /// The event-monitor updated list of states we use to check the job's status
    states         : Arc<DashMap<String, JobStatus>>,

    /// The timeout before we call it a day
    timeout          : u128,
    /// The time since the last check
    timeout_start    : SystemTime,
}

impl Future for WaitUntilNewState {
    type Output = Option<(JobStatus, SystemTime)>;

    /// Polls the WaitUntilCompleted to see if the remote job has been completed (or failed to do so).
    /// 
    /// **Arguments**
    ///  * `cx`: The context with which to check if we need to wait for something.
    /// 
    /// **Returns**  
    /// A Poll::Ready with the JobStatus we found and the time we found it at, or a Poll::Ready with None if we timed out.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Try to match the current state of the job
        let state = self.states.get(&self.correlation_id);
        if let Some(state) = state {
            let state = state.value();
            if std::mem::discriminant(state) != std::mem::discriminant(&self.current_state) {
                // It has changed
                return Poll::Ready(Some((state.clone(), SystemTime::now())));
            }
        }

        // Get the time since the last update
        let last_update: SystemTime = match &self.heartbeats {
            Some(heartbeats) => match heartbeats.get(&self.correlation_id) {
                Some(last_update) => *last_update.value(),
                None              => self.timeout_start,
            },
            None => self.timeout_start,
        };

        // Compute how many milliseconds passed since the start
        let elapsed = match SystemTime::now().duration_since(last_update) {
            Ok(elapsed) => elapsed,
            Err(err)    => { panic!("The time since we last saw a heartbeat is later than the current time (by {:?}); this should never happen!", err.duration()); }
        };

        // If we haven't seen the event on time, report a timeout (a None)
        if elapsed.as_millis() >= self.timeout { Poll::Ready(None) }
        else {
            // Keep trying
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}





/***** HELPER FUNCTIONS *****/
/// Waits until the job with the given correlation ID is created.
/// 
/// **Arguments**
///  * `correlation_id`: The ID of the job to wait for.
///  * `states`: The list of states to use for checking the job's progress (maintained by the event monitor).
/// 
/// **Returns**  
/// Nothing on success, or a ScheduleError if the job didn't make creation.
async fn job_wait_created(correlation_id: &str, states: Arc<DashMap<String, JobStatus>>) -> Result<(), ScheduleError> {
    // Wait for a change in state
    let new_state = WaitUntilNewState {
        correlation_id : correlation_id.to_string(),
        current_state  : JobStatus::Unknown,

        heartbeats : None,
        states     : states.clone(),

        timeout          : DEFAULT_CREATED_TIMEOUT,
        timeout_start    : SystemTime::now(),
    }.await;

    // Now match the new state
    match new_state {
        // If we failed to create, throw that error
        Some((JobStatus::CreateFailed{ err }, _))     => Err(ScheduleError::JobCreateFailed{ correlation_id: correlation_id.to_string(), err }),

        // For literally any other state, we're done
        Some(_) => Ok(()),

        // If we see 'None', then a timeout occurred
        None => Err(ScheduleError::JobCreatedTimeout{ correlation_id: correlation_id.to_string() }),
    }
}

/// Waits until the job with the given correlation ID is created, started and then finished.
/// 
/// **Arguments**
///  * `correlation_id`: The ID of the job to wait for.
///  * `heartbeats`: The list of heartbeats to use for checking the job's alive status (maintained by the event monitor).
///  * `states`: The list of states to use for checking the job's progress (maintained by the event monitor).
/// 
/// **Returns**  
/// The job's return value on success, or a ScheduleError if the job didn't make creation.
async fn job_wait_finished(correlation_id: &str, heartbeats: Arc<DashMap<String, SystemTime>>, states: Arc<DashMap<String, JobStatus>>) -> Result<Value, ScheduleError> {
    // Jeep iterating until, inevitably, we timeout, see an error or see a finished state
    let mut last_state       = JobStatus::Unknown;
    let mut last_time_update = SystemTime::now();
    loop {
        // Determine the timeout based on the state
        let timeout = match last_state {
            JobStatus::Unknown     => DEFAULT_CREATED_TIMEOUT,
            JobStatus::Created     => DEFAULT_READY_TIMEOUT,
            JobStatus::Ready       => DEFAULT_INITIALIZED_TIMEOUT,
            JobStatus::Initialized => DEFAULT_STARTED_TIMEOUT,
            JobStatus::Started     => DEFAULT_HEARTBEAT_TIMEOUT,
            JobStatus::Completed   => DEFAULT_RESULT_TIMEOUT,
            _                      => { unreachable!(); }
        };

        // Wait for a change in state
        let new_state = WaitUntilNewState {
            correlation_id : correlation_id.to_string(),
            current_state  : last_state.clone(),

            heartbeats : if std::mem::discriminant(&last_state) == std::mem::discriminant(&JobStatus::Started) { Some(heartbeats.clone()) } else { None },
            states     : states.clone(),

            timeout,
            timeout_start    : last_time_update,
        }.await;

        // Now match the new state
        match new_state {
            // If it's any of the final states, then we can quit
            Some((JobStatus::Finished{ res }, _)) => {
                // Try to parse as a Value
                match serde_json::from_str::<Value>(&res) {
                    Ok(value) => { return Ok(value); },
                    Err(err)  => { return Err(ScheduleError::FinishedDeserializeError{ output: res, err }); },
                }
            },
            Some((JobStatus::Failed{ res }, _)) => {
                // Try to parse as a FailureResult
                match serde_json::from_str::<FailureResult>(&res) {
                    Ok(result) => { return Err(ScheduleError::JobFailed{ correlation_id: correlation_id.to_string(), code: result.code, stdout: result.stdout, stderr: result.stderr }); },
                    Err(err)   => { return Err(ScheduleError::FailedDeserializeError{ output: res, err }); },
                }
            },
            Some((JobStatus::Stopped{ signal }, _))   => { return Err(ScheduleError::JobStopped{ correlation_id: correlation_id.to_string(), signal }); },
            Some((JobStatus::DecodeFailed{ err }, _)) => { return Err(ScheduleError::JobDecodeFailed{ correlation_id: correlation_id.to_string(), err }); },

            // Otherwise, for any other error, quit as well
            Some((JobStatus::CompleteFailed{ err }, _))   => { return Err(ScheduleError::JobCompleteFailed{ correlation_id: correlation_id.to_string(), err }) },
            Some((JobStatus::StartFailed{ err }, _))      => { return Err(ScheduleError::JobStartFailed{ correlation_id: correlation_id.to_string(), err }) },
            Some((JobStatus::InitializeFailed{ err }, _)) => { return Err(ScheduleError::JobInitializeFailed{ correlation_id: correlation_id.to_string(), err }) },
            Some((JobStatus::CreateFailed{ err }, _))     => { return Err(ScheduleError::JobCreateFailed{ correlation_id: correlation_id.to_string(), err }) },

            // For any other state, set it as the last state and see if we need to match again
            Some((new_state, time_update)) => { last_state = new_state; last_time_update = time_update; }

            // If we see 'None', then a timeout occurred
            None => {
                // Depending on the order of the last state, do different timeout error
                if      last_state.order() == JobStatus::Unknown.order()     { return Err(ScheduleError::JobCreatedTimeout{ correlation_id: correlation_id.to_string() }); }
                else if last_state.order() == JobStatus::Created.order()     { return Err(ScheduleError::JobReadyTimeout{ correlation_id: correlation_id.to_string() }); }
                else if last_state.order() == JobStatus::Ready.order()       { return Err(ScheduleError::JobInitializedTimeout{ correlation_id: correlation_id.to_string() }); }
                else if last_state.order() == JobStatus::Initialized.order() { return Err(ScheduleError::JobStartedTimeout{ correlation_id: correlation_id.to_string() }); }
                else if last_state.order() == JobStatus::Started.order()     { return Err(ScheduleError::JobHeartbeatTimeout{ correlation_id: correlation_id.to_string() }); }
                else if last_state.order() == JobStatus::Completed.order()   { return Err(ScheduleError::JobResultTimeout{ correlation_id: correlation_id.to_string() }); }
                else { unreachable!(); }
            },
        }

        // Do a nice debug print
        debug!("Job '{}' reached state {:?}", correlation_id, last_state);
    }
}





/***** DRIVER EXECUTOR *****/
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
    pub heartbeats: Arc<DashMap<String, SystemTime>>,
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
        let image = format!("{}:{}@{}", function.package, function.version, function.digest);
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
        if let Err(err) = self.producer.send(message, timeout).await {
            return Err(ExecutorError::CommandScheduleError{ topic: self.command_topic.clone(), err: format!("{:?}", err) });
        }

        if function.detached {
            // It's a detached, so we only wait until it's underway
            let created = job_wait_created(&correlation_id, self.states.clone());

            info!("Waiting until (detached) job '{}' is created...", correlation_id);
            let res = created.await;
            if let Err(err) = res {
                return Err(ExecutorError::ExternalCallError{ name: function.name, package: function.package, version: function.version, err: format!("{}", err) });
            }
            info!("OK, job '{}' has been created", correlation_id);

            // Return a Service that represents the running call
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
            // Wait until the job is completed
            let finished = job_wait_finished(&correlation_id, self.heartbeats.clone(), self.states.clone());

            info!("Waiting until job '{}' is finished...", correlation_id);
            let value = match finished.await {
                Ok(value) => value,
                Err(ScheduleError::JobFailed{ code, stdout, stderr, .. }) => { return Err(ExecutorError::ExternalCallFailed{ name: function.name, package: function.package, version: function.version, code, stdout, stderr }); }
                Err(err) => { return Err(ExecutorError::ExternalCallError{ name: function.name, package: function.package, version: function.version, err: format!("{}", err) }); }
            };
            info!("OK, job '{}' is finished", correlation_id);

            // Remove the job
            self.states.remove(&correlation_id);

            // Return the result
            debug!("RESULT: {:?}", value);
            Ok(value)
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

        // use try_send instead, since we don't _really_ care if the debug message doesn't go to the other side
        if let Err(reason) = self.client_tx.try_send(Ok(reply)) {
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

        // Use a timeout of say a minute
        if let Err(reason) = tokio::time::timeout(std::time::Duration::from_secs(60), self.client_tx.send(Ok(reply))).await {
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

        // Use a timeout of say a minute
        if let Err(reason) = tokio::time::timeout(std::time::Duration::from_secs(60), self.client_tx.send(Ok(reply))).await {
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





/***** FUTURES *****/
// /// Waits until the job with the given correlation ID is ready to stand on its own (created).
// struct WaitUntilCreated {
//     /// The correlation ID of the job we're waiting for
//     correlation_id : String,
//     /// The event-monitor updated list of states we use to check the job's status
//     states         : Arc<DashMap<String, JobStatus>>,
//     /// The last time anything of significance occurred
//     last_update    : SystemTime,
// }

// impl WaitUntilCreated {
//     /// Constructor for the WaitUntilCreated.
//     /// 
//     /// **Arguments**
//     ///  * `correlation_id`: The correlation ID of the job we're waiting for.
//     ///  * `states`: The event-monitor updated list of states we use to check the job's status.
//     pub fn new(correlation_id: String, states: Arc<DashMap<String, JobStatus>>) -> Self {
//         WaitUntilCreated {
//             correlation_id,
//             states,
//             last_update : SystemTime::now(),
//         }
//     }
// }

// impl Future for WaitUntilCreated {
//     type Output = Result<SystemTime, ScheduleError>;

//     /// Polls the WaitUntilCreated to see if the remote job has been started.
//     /// 
//     /// **Arguments**
//     ///  * `cx`: The context with which to check if we need to wait for something.
//     /// 
//     /// **Returns**  
//     /// A Poll::Ready with the time we received the Created event if it has started, or a Poll::Ready with a ExecutorError upon failure.
//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         // Try to see if a state is already known about the job
//         let state = self.states.get(&self.correlation_id);

//         // Match on the value found
//         match state {
//             Some(state) => {
//                 let state = state.value();
//                 match state {
//                     JobStatus::CreateFailed{ err } => {
//                         // That didn't go well
//                         return Poll::Ready(Err(ScheduleError::JobCreateFailed{ correlation_id: self.correlation_id.clone(), err: err.clone() }));
//                     },

//                     _ => {
//                         // Literally any other status is OK to be returned as 'created'
//                         return Poll::Ready(Ok(SystemTime::now()));
//                     }
//                 }
//             },
//             _ => {}
//         };

//         // Compute how many milliseconds passed since last status update
//         let elapsed = match SystemTime::now().duration_since(self.last_update) {
//             Ok(elapsed) => elapsed,
//             Err(err)    => { panic!("The time the job started is later than the current time (by {:?}); this should never happen!", err.duration()); }
//         };

//         // If we haven't seen the Created event on time, throw a tantrum
//         if elapsed.as_millis() >= DEFAULT_CREATED_TIMEOUT { Poll::Ready(Err(ScheduleError::JobCreatedTimeout{ correlation_id: self.correlation_id.clone(), timeout_ms: DEFAULT_CREATED_TIMEOUT })) }
//         else {
//             // Keep trying
//             cx.waker().wake_by_ref();
//             Poll::Pending
//         }
//     }
// }



// /// Waits until the job with the given correlation ID is up and running.
// struct WaitUntilStarted {
//     /// The correlation ID of the job we're waiting for
//     correlation_id : String,
//     /// The event-monitor updated list of states we use to check the job's status
//     states         : Arc<DashMap<String, JobStatus>>,
//     /// The last state we saw of the job
//     last_state     : JobStatus,
//     /// The last time anything of significance occurred
//     last_update    : SystemTime,
// }

// impl WaitUntilStarted {
//     /// Constructor for the WaitUntilStarted.
//     /// 
//     /// **Arguments**
//     ///  * `correlation_id`: The correlation ID of the job we're waiting for.
//     ///  * `states`: The event-monitor updated list of states we use to check the job's status.
//     ///  * `create_time`: The time that we received the job has been created (used for timing out).
//     pub fn new(correlation_id: String, states: Arc<DashMap<String, JobStatus>>, create_time: SystemTime) -> Self {
//         WaitUntilStarted {
//             correlation_id,
//             states,
//             last_state  : JobStatus::Created,
//             last_update : create_time,
//         }
//     }
// }

// impl Future for WaitUntilStarted {
//     type Output = Result<SystemTime, ScheduleError>;

//     /// Polls the WaitUntilStarted to see if the remote job has been started.  
//     /// We assume that we'll never see the 'Created' and 'CreateFailed' statusses, but will simply wait for them indefinitely if we do.
//     /// 
//     /// **Arguments**
//     ///  * `cx`: The context with which to check if we need to wait for something.
//     /// 
//     /// **Returns**  
//     /// A Poll::Ready with the time we received the Started event if it has started, or a Poll::Ready with a ExecutorError upon failure.
//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         self.as_mut().last_state = JobStatus::Unknown;
//         self.as_ref().last_state = JobStatus::Unknown;


//         // Get the muteable parts
//         let Self {
//             ref correlation_id,
//             ref states,
//             ref mut last_state,
//             ref mut last_update,
//         } = *self;

//         // Try to see if a state is already known about the job
//         let state = states.get(correlation_id);

//         // Match on the value found
//         match state {
//             Some(state) => {
//                 let state = state.value().clone();
//                 match state {
//                     JobStatus::Ready => {
//                         // The branelet itself now told us it has initialized to the point of communication
//                         if *last_state != state {
//                             *last_state  = state;
//                             *last_update = SystemTime::now();
//                         }
//                     }

//                     JobStatus::InitializeFailed{ err } => {
//                         // That didn't go well
//                         return Poll::Ready(Err(ScheduleError::JobInitializeFailed{ correlation_id: correlation_id.clone(), err: err.clone() }));
//                     }
//                     JobStatus::Initialized => {
//                         // The branelet has prepared launching the job
//                         if *last_state != state {
//                             *last_state  = state;
//                             *last_update = SystemTime::now();
//                         }
//                     }

//                     JobStatus::StartFailed{ err } => {
//                         // That didn't go well at all
//                         return Poll::Ready(Err(ScheduleError::JobInitializeFailed{ correlation_id: correlation_id.clone(), err: err.clone() }));
//                     }
//                     JobStatus::Started => {
//                         // That's it! We're done with waiting for this job
//                         return Poll::Ready(Ok(SystemTime::now()));
//                     }

//                     _ => {
//                         // If we happen to see an event that's later than the current one, we also stop waiting
//                         if !state.is_starting() { return Poll::Ready(Ok(SystemTime::now())); }
//                     }
//                 }
//             },
//             _ => {}
//         };

//         // Compute how many milliseconds passed since last status update
//         let elapsed = match SystemTime::now().duration_since(*last_update) {
//             Ok(elapsed) => elapsed,
//             Err(err)    => { panic!("The time the job sent its most recent status update is later than the current time (by {:?}); this should never happen!", err.duration()); }
//         };

//         // Return the appropriate poll depending on whether a timeout occurred or not
//         if *last_state == JobStatus::Created && elapsed.as_millis() >= DEFAULT_READY_TIMEOUT { Poll::Ready(Err(ScheduleError::JobReadyTimeout{ correlation_id: correlation_id.clone(), timeout_ms: DEFAULT_READY_TIMEOUT })) }
//         else if *last_state == JobStatus::Ready && elapsed.as_millis() >= DEFAULT_INITIALIZED_TIMEOUT{ Poll::Ready(Err(ScheduleError::JobInitializedTimeout{ correlation_id: correlation_id.clone(), timeout_ms: DEFAULT_INITIALIZED_TIMEOUT })) }
//         else if *last_state == JobStatus::Initialized && elapsed.as_millis() >= DEFAULT_STARTED_TIMEOUT{ Poll::Ready(Err(ScheduleError::JobStartedTimeout{ correlation_id: correlation_id.clone(), timeout_ms: DEFAULT_STARTED_TIMEOUT })) }
//         else {
//             // Keep trying
//             cx.waker().wake_by_ref();
//             Poll::Pending
//         }
//     }
// }



// /// Waits until the job with the given correlation ID has completed.
// struct WaitUntilFinished {
//     /// The correlation ID of the job we're waiting for
//     correlation_id : String,
//     /// The event-monitor updated list of states we use to check the job's status
//     states         : Arc<DashMap<String, JobStatus>>,
//     /// The event-monitor updated list of last heartbeats for jobs
//     heartbeats     : Arc<DashMap<String, SystemTime>>,
//     /// The last state we saw of the job
//     last_state     : JobStatus,
//     /// The time the job started
//     started        : SystemTime,
// }

// impl WaitUntilFinished {
//     /// Constructor for the WaitUntilFinished.
//     /// 
//     /// **Arguments**
//     ///  * `correlation_id`: The correlation ID of the job we're waiting for.
//     ///  * `states`: The event-monitor updated list of states we use to check the job's status.
//     ///  * `start_time`: The time that we received the job has started (used for timing out).
//     pub fn new(correlation_id: String, states: Arc<DashMap<String, JobStatus>>, heartbeats: Arc<DashMap<String, SystemTime>>, start_time: SystemTime) -> Self {
//         WaitUntilFinished {
//             correlation_id,
//             states,
//             heartbeats,
//             last_state : JobStatus::Started,
//             started    : start_time,
//         }
//     }
// }

// impl Future for WaitUntilFinished {
//     type Output = Result<Value, ScheduleError>;

//     /// Polls the WaitUntilFinished to see if the remote job has been started.  
//     /// We assume that we'll never see any starting statusses, but will simply wait for them indefinitely if we do.
//     /// 
//     /// **Arguments**
//     ///  * `cx`: The context with which to check if we need to wait for something.
//     /// 
//     /// **Returns**  
//     /// A Poll::Ready with the result we received from the call if it has finished, or a Poll::Ready with a ExecutorError upon failure.
//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         // Try to see if a state is already known about the job
//         let state = self.states.get(&self.correlation_id);

//         // Match on the value found
//         match state {
//             Some(state) => {
//                 let state = state.value();
//                 match state {
//                     JobStatus::Stopped => {
//                         // The job was stopped externally
//                         return Poll::Ready(Err(ScheduleError::JobStopped{ correlation_id: self.correlation_id.clone() }));   
//                     }
//                     JobStatus::Failed{ code, stdout, stderr } => {
//                         // The job failed, apparently
//                         return Poll::Ready(Err(ScheduleError::JobFailed{ correlation_id: self.correlation_id.clone(), code: *code, stdout: stdout.clone(), stderr: stderr.clone() }));
//                     }
//                     JobStatus::Finished{ res } => {
//                         // That looks good!
//                         return Poll::Ready(Ok(res.clone()));
//                     }

//                     _ => {}
//                 }
//             },
//             _ => {}
//         };

//         // Try to get the time since the last heartbeat
//         let heartbeat = match self.heartbeats.get(&self.correlation_id) {
//             Some(heartbeat) => heartbeat.value().clone(),
//             None            => self.started.clone(),
//         };
//         let elapsed = match SystemTime::now().duration_since(heartbeat) {
//             Ok(elapsed) => elapsed,
//             Err(err)    => { panic!("The time the job sent its last heartbeat is later than the current time (by {:?}); this should never happen!", err.duration()); }
//         };

//         // Keep trying or return a timeout if we haven't seen heartbeats in too long
//         if elapsed.as_millis() >= DEFAULT_HEARTBEAT_TIMEOUT { Poll::Ready(Err(ScheduleError::JobHeartbeatTimeout{ correlation_id: self.correlation_id.clone(), timeout_ms: DEFAULT_HEARTBEAT_TIMEOUT })) }
//         else {
//             // Keep trying
//             cx.waker().wake_by_ref();
//             Poll::Pending
//         }
//     }
// }
