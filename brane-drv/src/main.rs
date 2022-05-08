use anyhow::{Context, Result};
use brane_bvm::vm::VmState;
use brane_cfg::Infrastructure;
use brane_drv::errors::DriverError;
use brane_drv::grpc::DriverServiceServer;
use brane_drv::handler::DriverHandler;
use brane_job::interface::{Event, EventKind};
use brane_shr::jobs::JobStatus;
use clap::Parser;
use dashmap::DashMap;
use dotenv::dotenv;
use futures::TryStreamExt;
use log::info;
use log::LevelFilter;
use prost::Message as _;
use rdkafka::{
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    consumer::{Consumer, StreamConsumer},
    error::RDKafkaErrorCode,
    producer::FutureProducer,
    util::Timeout,
    ClientConfig, Message as _, Offset, TopicPartitionList
};
use std::sync::Arc;
use std::time::SystemTime;
use tonic::transport::Server;


/***** ARGUMENTS *****/
#[derive(Parser)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Opts {
    /// GraphQL address
    #[clap(long, default_value = "http://127.0.0.1:50051/graphql", env = "GRAPHQL_URL")]
    graphql_url: String,
    /// Service address
    #[clap(short, long, default_value = "127.0.0.1:50053", env = "ADDRESS")]
    address: String,
    /// Kafka brokers
    #[clap(short, long, default_value = "localhost:9092", env = "BROKERS")]
    brokers: String,
    /// Topic to send commands to
    #[clap(short, long = "cmd-topic", default_value = "drv-cmd", env = "COMMAND_TOPIC")]
    command_topic: String,
    /// Topic to recieve events from
    #[clap(short, long = "evt-topic", default_value = "job-evt", env = "EVENT_TOPIC")]
    event_topic: String,
    /// Print debug info
    #[clap(short, long, env = "DEBUG", takes_value = false)]
    debug: bool,
    /// Consumer group id
    #[clap(short, long, default_value = "brane-drv")]
    group_id: String,
    /// Infra metadata store
    #[clap(short, long, default_value = "./infra.yml", env = "INFRA")]
    infra: String,
}
/*******/





/***** ENTRY POINT *****/
#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let opts = Opts::parse();

    // Configure logger.
    let mut logger = env_logger::builder();
    logger.format_module_path(false);

    if opts.debug {
        logger.filter_level(LevelFilter::Debug).init();
    } else {
        logger.filter_level(LevelFilter::Info).init();
    }

    // Ensure that the input/output topics exists.
    let command_topic = opts.command_topic.clone();
    if let Err(reason) = ensure_topics(vec![&command_topic, &opts.event_topic], &opts.brokers).await {
        log::error!("{}", reason);
        std::process::exit(-1);
    };

    let infra = Infrastructure::new(opts.infra.clone())?;
    infra.validate()?;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &opts.brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .context("Failed to create Kafka producer.")?;

    // Start event monitor in the background.
    let states: Arc<DashMap<String, JobStatus>> = Arc::new(DashMap::new());
    let heartbeats: Arc<DashMap<String, SystemTime>> = Arc::new(DashMap::new());
    let locations: Arc<DashMap<String, String>> = Arc::new(DashMap::new());

    tokio::spawn(start_event_monitor(
        opts.brokers.clone(),
        opts.group_id.clone(),
        opts.event_topic.clone(),
        states.clone(),
        heartbeats.clone(),
        locations.clone(),
    ));

    let graphql_url = opts.graphql_url.clone();
    let sessions: Arc<DashMap<String, VmState>> = Arc::new(DashMap::new());
    let handler = DriverHandler {
        command_topic,
        graphql_url,
        producer,
        sessions,
        states,
        heartbeats,
        locations,
        infra,
    };

    // Start gRPC server with callback service.
    Server::builder()
        .add_service(DriverServiceServer::new(handler))
        .serve(opts.address.parse()?)
        .await
        .context("Failed to start callback gRPC server.")
}

/* TIM */
/// **Edited: now returning DriverErrors.**
///
/// Makes sure the required topics are present and watched in the local Kafka server.
/// 
/// **Arguments**
///  * `topics`: The list of topics to make sure they exist of.
///  * `brokers`: The string list of Kafka servers that act as the brokers.
/// 
/// **Returns**  
/// Nothing on success, or a DriverError otherwise.
async fn ensure_topics(
    topics: Vec<&str>,
    brokers: &str,
) -> Result<(), DriverError> {
    // Connect with an admin client
    let admin_client: AdminClient<_> = match ClientConfig::new().set("bootstrap.servers", brokers) .create() {
        Ok(client)  => client,
        Err(reason) => { return Err(DriverError::KafkaClientError{ servers: brokers.to_string(), err: reason }); }
    };

    // Collect the topics to create and then create them
    let ktopics: Vec<NewTopic> = topics
        .iter()
        .map(|t| NewTopic::new(t, 1, TopicReplication::Fixed(1)))
        .collect();
    let results = match admin_client.create_topics(ktopics.iter(), &AdminOptions::new()).await {
        Ok(results) => results,
        Err(reason) => { return Err(DriverError::KafkaTopicsError{ topics: DriverError::serialize_vec(&topics), err: reason }); }
    };

    // Report on the results. Don't consider 'TopicAlreadyExists' an error.
    for result in results {
        match result {
            Ok(topic) => info!("Kafka topic '{}' created.", topic),
            Err((topic, error)) => match error {
                RDKafkaErrorCode::TopicAlreadyExists => {
                    info!("Kafka topic '{}' already exists", topic);
                }
                _ => { return Err(DriverError::KafkaTopicError{ topic, err: error }); }
            },
        }
    }

    Ok(())
}
/*******/

/* TIM */
/// **Edited: taking into account new events. To do so, now accepting 'heartbeats' list.**
/// 
/// Monitors the Kafka events for interesting stuff for us.
/// 
/// **Arguments**
///  * `brokers`: The list of Kafka servers to listen to.
///  * `group_id`: The group_id for the brane-drv.
///  * `topic`: The topic to listen on.
///  * `states`: The list of states we use to keep track at what state what running job is.
///  * `heartbeats`: The list of times we last saw a heartbeat for a given job.
///  * `results`: A list to put the results in we accumulated from each job.
///  * `locations`: The list of locations where our jobs are running.
/// 
/// **Returns**  
/// Nothing on success, or a DriverError upon failure.
async fn start_event_monitor(
    brokers: String,
    group_id: String,
    topic: String,
    states: Arc<DashMap<String, JobStatus>>,
    heartbeats: Arc<DashMap<String, SystemTime>>,
    locations: Arc<DashMap<String, String>>,
) -> Result<(), DriverError> {
    let consumer: StreamConsumer = match ClientConfig::new()
        .set("group.id", group_id.clone())
        .set("bootstrap.servers", brokers.clone())
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .create()
    {
        Ok(consumer) => consumer,
        Err(err)     => { return Err(DriverError::KafkaConsumerError{ servers: brokers, id: group_id, err }); }
    };

    // Restore previous topic/partition offset.
    let mut tpl = TopicPartitionList::new();
    tpl.add_partition(&topic, 0);

    let committed_offsets = match consumer.committed_offsets(tpl.clone(), Timeout::Never) {
        Ok(commited_offsets) => commited_offsets.to_topic_map(),
        Err(err)             => { return Err(DriverError::KafkaGetOffsetError{ topic, err }); }
    };
    if let Some(offset) = committed_offsets.get(&(topic.clone(), 0)) {
        let res = match offset {
            Offset::Invalid => tpl.set_partition_offset(&topic, 0, Offset::Beginning),
            offset => tpl.set_partition_offset(&topic, 0, *offset),
        };
        if let Err(err) = res {
            return Err(DriverError::KafkaSetOffsetError{ topic, err });
        }
    }

    info!("Restoring commited offsets: {:?}", &tpl);
    if let Err(err) = consumer.assign(&tpl) {
        return Err(DriverError::KafkaSetOffsetsError{ topic, err });
    }

    // Run the consumer
    match consumer
        .stream()
        .try_for_each(|borrowed_message| {
            let owned_message = borrowed_message.detach();
            let owned_states = states.clone();
            let owned_heartbeats = heartbeats.clone();
            let owned_locations = locations.clone();

            async move {
                if let Some(payload) = owned_message.payload() {
                    // Decode payload into a Event message.
                    let event = Event::decode(payload).unwrap();
                    let kind = EventKind::from_i32(event.kind).unwrap();

                    let event_id: Vec<_> = event.identifier.split('-').collect();
                    let correlation_id = event_id.first().unwrap().to_string();

                    // Just collect everything we see; don't reason about it yet
                    match kind {
                        EventKind::CreateFailed => {
                            // Decode the payload as error
                            let err = String::from_utf8_lossy(&event.payload).to_string();
                            // Note the state with what went wrong
                            owned_states.insert(correlation_id, JobStatus::CreateFailed{ err });
                        }
                        EventKind::Created => {
                            // The container has been created, so note it
                            owned_states.insert(correlation_id.clone(), JobStatus::Created);
                            owned_locations.insert(correlation_id, event.location.clone());
                        }

                        EventKind::Ready => {
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::Ready);
                        }

                        EventKind::InitializeFailed => {
                            // Decode the payload as error
                            let err = String::from_utf8_lossy(&event.payload).to_string();
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::InitializeFailed{ err });
                        }
                        EventKind::Initialized => {
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::Initialized);
                        }

                        EventKind::StartFailed => {
                            // Decode the payload as error
                            let err = String::from_utf8_lossy(&event.payload).to_string();
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::StartFailed{ err });
                        }
                        EventKind::Started => {
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::Started);
                        }

                        EventKind::Heartbeat => {
                            // Note the time that we received the heartbeat only
                            owned_heartbeats.insert(correlation_id, SystemTime::now());
                        }
                        EventKind::CompleteFailed => {
                            // Decode the payload as error
                            let err = String::from_utf8_lossy(&event.payload).to_string();
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::CompleteFailed{ err });
                        }
                        EventKind::Completed => {
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::Completed);
                        }

                        EventKind::DecodeFailed => {
                            // Decode the payload as error
                            let err = String::from_utf8_lossy(&event.payload).to_string();
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::DecodeFailed{ err });
                        }
                        EventKind::Failed => {
                            // Decode the result as a JSON code/stdout/stderr pair
                            let payload = String::from_utf8_lossy(&event.payload).to_string();
                            // Do not parse the JSON, as this is error-prone and we want to treat errors in the executor
                            owned_states.insert(correlation_id, JobStatus::Failed{ res: payload });
                        }
                        EventKind::Stopped => {
                            // Decode the payload as a signal name
                            let signal = String::from_utf8_lossy(&event.payload).to_string();
                            // Update the state
                            owned_states.insert(correlation_id, JobStatus::Stopped{ signal });
                        }
                        EventKind::Finished => {
                            // Decode the payload as JSON value description
                            let payload = String::from_utf8_lossy(&event.payload).to_string();
                            // Do not parse the JSON, as this is error-prone and we want to treat errors in the executor
                            owned_states.insert(correlation_id, JobStatus::Finished{ res: payload });
                        }
                        _ => {
                            unreachable!();
                        }
                    }
                }

                Ok(())
            }
        })
        .await
    {
        Ok(_)    => Ok(()),
        Err(err) => Err(DriverError::EventMonitorError{ err }),
    }
}
/*******/
