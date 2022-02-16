use crate::executor::JobExecutor;
use crate::{grpc, packages};
use anyhow::Result;
use brane_bvm::vm::{Vm, VmOptions, VmState, VmError};
use brane_cfg::Infrastructure;
use brane_dsl::{Compiler, CompilerOptions, Lang};
use brane_shr::jobs::JobStatus;
use dashmap::DashMap;
use rdkafka::producer::FutureProducer;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use uuid::Uuid;

#[derive(Clone)]
pub struct DriverHandler {
    pub command_topic: String,
    pub graphql_url: String,
    pub producer: FutureProducer,
    pub sessions: Arc<DashMap<String, VmState>>,
    pub states: Arc<DashMap<String, JobStatus>>,
    pub heartbeats: Arc<DashMap<String, SystemTime>>,
    pub locations: Arc<DashMap<String, String>>,
    pub infra: Infrastructure,
}

#[tonic::async_trait]
impl grpc::DriverService for DriverHandler {
    type ExecuteStream = ReceiverStream<Result<grpc::ExecuteReply, Status>>;

    ///
    ///
    ///
    async fn create_session(
        &self,
        _request: Request<grpc::CreateSessionRequest>,
    ) -> Result<Response<grpc::CreateSessionReply>, Status> {
        let uuid = Uuid::new_v4().to_string();

        let reply = grpc::CreateSessionReply { uuid };
        Ok(Response::new(reply))
    }

    ///
    ///
    ///
    async fn execute(
        &self,
        request: Request<grpc::ExecuteRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let request = request.into_inner();
        let package_index = packages::get_package_index(&self.graphql_url).await.unwrap();
        let sessions = self.sessions.clone();

        // Prepare gRPC stream between client and (this) driver.
        let (tx, rx) = mpsc::channel::<Result<grpc::ExecuteReply, Status>>(10);

        let executor = JobExecutor {
            client_tx: tx.clone(),
            command_topic: self.command_topic.clone(),
            producer: self.producer.clone(),
            session_uuid: request.uuid.clone(),
            states: self.states.clone(),
            heartbeats: self.heartbeats.clone(),
            locations: self.locations.clone(),
            infra: self.infra.clone(),
        };

        /* TIM */
        let vm_state = sessions.get(&request.uuid).as_deref().cloned();
        tokio::spawn(async move {
            let options = CompilerOptions::new(Lang::BraneScript);
            let mut compiler = Compiler::new(options, package_index.clone());

            // Compile input and send update to client.
            let function = match compiler.compile(request.input) {
                Ok(function) => function,
                Err(error) => {
                    let status = Status::invalid_argument(error.to_string());
                    tx.send(Err(status)).await.unwrap();
                    return;
                }
            };

            // Restore VM state corresponding to the session, if any.
            // We do this in a block to make sure vm doesn't exist anymore when we .await on tx.send
            let res: Result<(), VmError> = {
                // Create the VM with state if we have one, or otherwise without
                let mut vm = if let Some(vm_state) = vm_state {
                    debug!("Restore VM with state:\n{:?}", vm_state);
                    match Vm::new_with_state(executor, Some(package_index), vm_state) {
                        Ok(vm)      => Ok(vm),
                        Err(reason) => Err(reason),
                    }
                } else {
                    debug!("No VM state to restore, creating new VM.");
                    let options = VmOptions {
                        clear_after_main: true,
                        ..Default::default()
                    };
                    match Vm::new_with(executor, Some(package_index), Some(options)) {
                        Ok(vm)      => Ok(vm),
                        Err(reason) => Err(reason),
                    }
                };

                // Switch on the creation state of the VM
                match vm {
                    Ok(ref mut vm) => {
                        // We can continue to run it

                        // TEMP: needed because the VM is not completely `send`.
                        // futures::executor::block_on(vm.main(function));
                        let res = futures::executor::block_on(vm.main(function));

                        // Already store the state of the VM before erroring to let Tokio allow the .await on tx.send
                        let vm_state = vm.capture_state();
                        sessions.insert(request.uuid, vm_state);

                        // Done
                        res
                    },
                    // We couldn't create it
                    Err(reason) => Err(reason),
                }
            };

            // Make vm a non-muteable reference so it allows the await
            match res {
                Ok(()) => {
                    // Send a debug message to client saying it all worked out
                    debug!("Completed execution.");

                    // Create the reply text
                    let msg = String::from("Driver completed execution.");
                    let reply = grpc::ExecuteReply {
                        close: true,
                        debug: Some(msg.clone()),
                        stderr: None,
                        stdout: None,
                    };

                    // Send it to the client
                    if let Err(err) = tx.send(Ok(reply)).await {
                        error!("Could not send debug message '{}' to client: {}", msg, err);
                        anyhow!("Failed to send gRPC error message to client.");
                    }
                },
                Err(err) => {
                    // Create the reply text
                    let msg = format!("{}", err);
                    let reply = grpc::ExecuteReply {
                        close: true,
                        debug: None,
                        stderr: Some(msg.clone()),
                        stdout: None,
                    };

                    // Send it to the client
                    if let Err(err) = tx.send(Ok(reply)).await {
                        error!("Could not send VM error '{}' to client: {}", msg, err);
                        anyhow!("Failed to send gRPC error message to client.");
                    }
                }
            }
        });
        /*******/

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
