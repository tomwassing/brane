use std::sync::Arc;

use anyhow::Result;
use brane_cfg::{Infrastructure, Secrets};
use brane_clb::interface::{Callback, CallbackKind};
use brane_job::{
    clb_lifecycle,
    interface::{Command, CommandKind, Event},
};
use brane_job::{cmd_create};
use brane_shr::utilities;
use bytes::BytesMut;
use brane_job::errors::JobError;
// use clap::Parser;
use structopt::StructOpt;
use dashmap::{lock::RwLock, DashMap};
use dotenv::dotenv;
use futures::stream::FuturesUnordered;
use futures::{StreamExt, TryStreamExt};
use log::LevelFilter;
use log::{debug, error, info, warn};
use prost::Message;
use rdkafka::{
    admin::{AdminClient, AdminOptions, NewTopic, TopicReplication},
    config::ClientConfig,
    consumer::{stream_consumer::StreamConsumer, CommitMode, Consumer},
    error::RDKafkaErrorCode,
    message::ToBytes,
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    Message as KafkaMesage, Offset, TopicPartitionList,
};
use tokio::task::JoinHandle;
use xenon::compute::Scheduler;

// #[derive(Parser)]
// #[clap(version = env!("CARGO_PKG_VERSION"))]
// struct Opts {
//     /// Topic to receive callbacks from
//     #[clap(short, long = "clb-topic", default_value = "clb", env = "CALLBACK_TOPIC")]
//     callback_topic: String,
//     /// Topic to receive commands from
//     #[clap(short = 'o', long = "cmd-topic", default_value = "plr-cmd", env = "COMMAND_TOPIC")]
//     command_topic: String,
//     /// Kafka brokers
//     #[clap(short, long, default_value = "127.0.0.1:9092", env = "BROKERS")]
//     brokers: String,
//     /// Print debug info
//     #[clap(short, long, env = "DEBUG", takes_value = false)]
//     debug: bool,
//     /// Topic to send events to
//     #[clap(short, long = "evt-topic", default_value = "job-evt", env = "EVENT_TOPIC")]
//     event_topic: String,
//     /// Consumer group id
//     #[clap(short, long, default_value = "brane-job", env = "GROUP_ID")]
//     group_id: String,
//     /// Infra metadata store
//     #[clap(short, long, default_value = "./infra.yml", env = "INFRA")]
//     infra: String,
//     /// Number of workers
//     #[clap(short = 'w', long, default_value = "1", env = "NUM_WORKERS")]
//     num_workers: u8,
//     /// Secrets store
//     #[clap(short, long, default_value = "./secrets.yml", env = "SECRETS")]
//     secrets: String,
//     /// Xenon gRPC endpoint
//     #[clap(short, long, default_value = "http://127.0.0.1:50051", env = "XENON")]
//     xenon: String,
// }

#[derive(StructOpt)]
struct Opts {
    /// Topic to receive callbacks from
    #[structopt(short, long = "clb-topic", default_value = "clb", env = "CALLBACK_TOPIC")]
    callback_topic: String,
    /// Topic to receive commands from
    #[structopt(short = "o", long = "cmd-topic", default_value = "plr-cmd", env = "COMMAND_TOPIC")]
    command_topic: String,
    /// Kafka brokers
    #[structopt(short, long, default_value = "127.0.0.1:9092", env = "BROKERS")]
    brokers: String,
    /// Print debug info
    #[structopt(short, long, env = "DEBUG", takes_value = false)]
    debug: bool,
    /// Topic to send events to
    #[structopt(short, long = "evt-topic", default_value = "job-evt", env = "EVENT_TOPIC")]
    event_topic: String,
    /// Consumer group id
    #[structopt(short, long, default_value = "brane-job", env = "GROUP_ID")]
    group_id: String,
    /// Infra metadata store
    #[structopt(short, long, default_value = "./infra.yml", env = "INFRA")]
    infra: String,
    /// Number of workers
    #[structopt(short = "w", long, default_value = "1", env = "NUM_WORKERS")]
    num_workers: u8,
    /// Secrets store
    #[structopt(short, long, default_value = "./secrets.yml", env = "SECRETS")]
    secrets: String,
    /// Xenon gRPC endpoint
    #[structopt(short, long, default_value = "http://127.0.0.1:50051", env = "XENON")]
    xenon: String,
}

/* TIM */
/// **Edited: Working with much more structured error handling.**
#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    // let opts = Opts::parse();
    let opts = Opts::from_args();

    // Configure logger.
    let mut logger = env_logger::builder();
    logger.format_module_path(false);

    if opts.debug {
        logger.filter_level(LevelFilter::Debug).init();
    } else {
        logger.filter_level(LevelFilter::Info).init();
    }
    debug!("Initializing brane-job...");

    // Ensure that the input/output topics exists.
    if let Err(reason) = ensure_topics(
        vec![&opts.callback_topic, &opts.command_topic, &opts.event_topic],
        &opts.brokers,
    ).await { error!("{}", reason); std::process::exit(-1); }

    debug!("Loading infrastructure file...");
    let infra = match Infrastructure::new(opts.infra.clone()) {
        Ok(infra)   => infra,
        Err(reason) => { error!("{}", reason); std::process::exit(-1); }
    };
    if let Err(reason) = infra.validate() { error!("{}", reason); std::process::exit(-1); }

    debug!("Loading secrets file...");
    let secrets = match Secrets::new(opts.secrets.clone()) {
        Ok(secrets) => secrets,
        Err(reason) => { error!("{}", reason); std::process::exit(-1); }
    };
    if let Err(reason) = secrets.validate() { error!("{}", reason); std::process::exit(-1); }

    debug!("Initializing Xenon...");
    let xenon_schedulers = Arc::new(DashMap::<String, Arc<RwLock<Scheduler>>>::new());
    let xenon_endpoint = utilities::ensure_http_schema(&opts.xenon, !opts.debug)?;

    // Spawn workers, using Tokio tasks and thread pool.
    debug!("Launching workers...");
    let workers = (0..opts.num_workers)
        .map(|i| {
            let handle = tokio::spawn(start_worker(
                opts.brokers.clone(),
                opts.group_id.clone(),
                opts.callback_topic.clone(),
                opts.command_topic.clone(),
                opts.event_topic.clone(),
                infra.clone(),
                secrets.clone(),
                xenon_endpoint.clone(),
                xenon_schedulers.clone(),
            ));

            info!("Spawned asynchronous worker #{}.", i + 1);
            handle
        })
        .collect::<FuturesUnordered<JoinHandle<_>>>();

    // Wait for workers to finish, print any errors.
    workers
        .map(|r| r.unwrap())
        .for_each(|r| async {
            if let Err(error) = r {
                error!("{}", error);
            };
        })
        .await;

    Ok(())
}
/*******/

/* TIM */
/// **Edited: now returns JobErrors.**
/// 
/// Makes sure the required topics are present and watched in the local Kafka server.
/// 
/// **Arguments**
///  * `topics`: The list of topics to make sure they exist of.
///  * `brokers`: The string list of Kafka servers that act as the brokers.
/// 
/// **Returns**  
/// Nothing on success, or an ExecutorError otherwise.
async fn ensure_topics(
    topics: Vec<&str>,
    brokers: &str,
) -> Result<(), JobError> {
    // Connect with an admin client
    let admin_client: AdminClient<_> = match ClientConfig::new().set("bootstrap.servers", brokers) .create() {
        Ok(client)  => client,
        Err(reason) => { return Err(JobError::KafkaClientError{ servers: brokers.to_string(), err: reason }); }
    };

    // Collect the topics to create and then create them
    let ktopics: Vec<NewTopic> = topics
        .iter()
        .map(|t| NewTopic::new(t, 1, TopicReplication::Fixed(1)))
        .collect();
    let results = match admin_client.create_topics(ktopics.iter(), &AdminOptions::new()).await {
        Ok(results) => results,
        Err(reason) => { return Err(JobError::KafkaTopicsError{ topics: JobError::serialize_vec(&topics), err: reason }); }
    };

    // Report on the results. Don't consider 'TopicAlreadyExists' an error.
    for result in results {
        match result {
            Ok(topic) => info!("Kafka topic '{}' created.", topic),
            Err((topic, error)) => match error {
                RDKafkaErrorCode::TopicAlreadyExists => {
                    info!("Kafka topic '{}' already exists", topic);
                }
                _ => { return Err(JobError::KafkaTopicError{ topic, err: error }); }
            },
        }
    }

    Ok(())
}
/*******/

/* TIM */
/// **Edited: Now working with the various errors.**
/// 
/// One of the workers in the brane-job service.
/// 
/// **Arguments**
///  * `brokers`: The list of Kafka brokers we're using.
///  * `group_id`: The Kafka group ID for the brane-job service.
///  * `clb_topic`: The Kafka callback topic for job results.
///  * `cmd_topic`: The Kafka command topic for incoming commands.
///  * `evt_topic`: The Kafka event topic where we report back to the driver.
///  * `infra`: The Infrastructure handle to the infra.yml.
///  * `secrets`: The Secrets handle to the infra.yml.
///  * `xenon_endpoint`: The Xenon endpoint to connect to and schedule jobs on.
///  * `xenon_schedulers`: A list of Xenon schedulers we use to determine where to run what.
/// 
/// **Returns**  
/// Nothing if the worker exited cleanly, or a JobError if it didn't.
#[allow(clippy::too_many_arguments)]
async fn start_worker(
    brokers: String,
    group_id: String,
    clb_topic: String,
    cmd_topic: String,
    evt_topic: String,
    infra: Infrastructure,
    secrets: Secrets,
    xenon_endpoint: String,
    xenon_schedulers: Arc<DashMap<String, Arc<RwLock<Scheduler>>>>,
) -> Result<(), JobError> {
    let output_topic = evt_topic.as_ref();

    debug!("Creating Kafka producer...");
    let producer: FutureProducer = match ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("message.timeout.ms", "5000")
        .create()
    {
        Ok(producer) => producer,
        Err(reason)  => { return Err(JobError::KafkaProducerError{ servers: brokers, err: reason }); }
    };

    debug!("Creating Kafka consumer...");
    let consumer: StreamConsumer = match ClientConfig::new()
        .set("group.id", &group_id)
        .set("bootstrap.servers", &brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "false")
        .create()
    {
        Ok(consumer) => consumer,
        Err(reason)  => { return Err(JobError::KafkaConsumerError{ servers: brokers, id: group_id, err: reason }); }
    };

    // TODO: make use of transactions / exactly-once semantics (EOS)

    // Restore previous topic/partition offset.
    let mut tpl = TopicPartitionList::new();
    tpl.add_partition(&clb_topic, 0);
    tpl.add_partition(&cmd_topic, 0);

    let committed_offsets = match consumer.committed_offsets(tpl.clone(), Timeout::Never) {
        Ok(commited_offsets) => commited_offsets.to_topic_map(),
        Err(reason)          => { return Err(JobError::KafkaGetOffsetError{ clb: clb_topic, cmd: cmd_topic, err: reason }); }
    };
    if let Some(offset) = committed_offsets.get(&(clb_topic.clone(), 0)) {
        let res = match offset {
            Offset::Invalid => tpl.set_partition_offset(&clb_topic, 0, Offset::Beginning),
            offset => tpl.set_partition_offset(&clb_topic, 0, *offset),
        };
        if let Err(reason) = res {
            return Err(JobError::KafkaSetOffsetError{ topic: clb_topic, kind: "callback".to_string(), err: reason });
        }
    }
    if let Some(offset) = committed_offsets.get(&(cmd_topic.clone(), 0)) {
        let res = match offset {
            Offset::Invalid => tpl.set_partition_offset(&cmd_topic, 0, Offset::Beginning),
            offset => tpl.set_partition_offset(&cmd_topic, 0, *offset),
        };
        if let Err(reason) = res {
            return Err(JobError::KafkaSetOffsetError{ topic: cmd_topic, kind: "command".to_string(), err: reason });
        }
    }

    info!("Restoring commited offsets: {:?}", &tpl);
    if let Err(reason) = consumer.assign(&tpl) {
        return Err(JobError::KafkaSetOffsetsError{ clb: clb_topic, cmd: cmd_topic, err: reason });
    }

    // Create the outer pipeline on the message stream.
    debug!("Waiting for messages...");
    let stream_processor = consumer.stream().try_for_each(|borrowed_message| {
        // Copy the message into owned space
        consumer.commit_message(&borrowed_message, CommitMode::Sync).unwrap();

        let owned_message = borrowed_message.detach();
        let owned_producer = producer.clone();
        let owned_infra = infra.clone();
        let owned_secrets = secrets.clone();
        let owned_xenon_endpoint = xenon_endpoint.clone();
        let owned_xenon_schedulers = xenon_schedulers.clone();
        let clb_topic = clb_topic.clone();
        let cmd_topic = cmd_topic.clone();

        async move {
            // Get the message key
            let msg_key = match owned_message
                .key()
                .map(String::from_utf8_lossy)
                .map(String::from)
            {
                Some(msg_key) => msg_key,
                None          => {
                    warn!("Received message without a key; ignoring message");
                    return Ok(());
                }
            };

            // Get the payload
            let msg_payload = match owned_message.payload() {
                Some(msg_payload) => msg_payload,
                None              => {
                    warn!("Received message (key: {}) without a payload; ignoring message", msg_key);
                    return Ok(());
                }
            };

            // Depending on the message's topic, handle it differently
            let topic = owned_message.topic();
            let events = if topic == clb_topic {
                handle_clb_message(msg_key, msg_payload)
            } else if topic == cmd_topic {
                handle_cmd_message(
                    msg_key,
                    msg_payload,
                    owned_infra,
                    owned_secrets,
                    owned_xenon_endpoint,
                    owned_xenon_schedulers,
                )
                .await
            } else {
                warn!("Received message (key: {}) with unknown topic '{}'; ignoring message", msg_key, topic);
                return Ok(());
            };

            // Match the events to return
            match events {
                Ok(events) => {
                    for (evt_key, event) in events {
                        // Encode event message into a payload (bytes)
                        let mut payload = BytesMut::with_capacity(64);
                        match event.encode(&mut payload) {
                            Ok(_) => {
                                // Send event on output topic
                                let message = FutureRecord::to(output_topic).key(&evt_key).payload(payload.to_bytes());
                                if let Err(error) = owned_producer.send(message, Timeout::Never).await {
                                    error!("Failed to send event (key: {}): {:?}", evt_key, error);
                                }
                            },
                            Err(reason) => { error!("Failed to send event (key: {}): {}", evt_key.clone(), JobError::EventEncodeError{ key: evt_key, err: reason }); }
                        }
                    }
                }
                Err(err) => {
                    // Log the error but continue listening
                    error!("{}", &err);
                }
            };

            Ok(())
        }
    });

    match stream_processor.await {
        Ok(_)  => Ok(()),
        Err(_) => panic!("The Stream Processor shouldn't return an error, but it does; this should never happen!"),
    }
}
/*******/

/* TIM */
/// **Edited: now returning JobErrors.**
/// 
/// Handles a given callback message by calling the appropriate handler.
/// 
/// **Arguments**
///  * `key`: The key of the message we received.
///  * `payload`: The raw, binary payload of the message.
/// 
/// **Returns**  
/// A list of events that should be fired on success, or a JobError if that somehow failed.
fn handle_clb_message(
    key: String,
    payload: &[u8],
) -> Result<Vec<(String, Event)>, JobError> {
    // Decode payload into a callback message.
    debug!("Decoding clb message...");
    let callback = match Callback::decode(payload) {
        Ok(callback) => callback,
        Err(reason)  => { return Err(JobError::CallbackDecodeError{ key, err: reason }); }
    };
    let kind = match CallbackKind::from_i32(callback.kind) {
        Some(kind) => kind,
        None       => { return Err(JobError::IllegalCallbackKind{ kind: callback.kind }); }
    };

    // Ignore unkown callbacks, as we can't dispatch it.
    if kind == CallbackKind::Unknown {
        warn!("Received UNKOWN command (key: {}); ignoring message", key);
        return Ok(vec![]);
    }

    info!("Received {} callback (key: {}).", kind, key);
    debug!("{:?}", callback);

    // Call the handlers
    match kind {
        // Do not handle the heartbeat separately, as we actually want it to reach the driver
        // CallbackKind::Heartbeat => clb_heartbeat::handle(callback),
        _ => clb_lifecycle::handle(callback),
    }
}
/*******/

/* TIM */
/// **Edited: now returning JobErrors.**
/// 
/// Handles a given command message by calling the appropriate handler.
/// 
/// **Arguments**
///  * `key`: The key of the message we received.
///  * `payload`: The raw, binary payload of the message.
///  * `infra`: The Infrastructure handle to the infra.yml.
///  * `secrets`: The Secrets handle to the infra.yml.
///  * `xenon_endpoint`: The Xenon endpoint to connect to and schedule jobs on.
///  * `xenon_schedulers`: A list of Xenon schedulers we use to determine where to run what.
/// 
/// **Returns**  
/// A list of events that should be fired on success, or a JobError if that somehow failed.
async fn handle_cmd_message(
    key: String,
    payload: &[u8],
    infra: Infrastructure,
    secrets: Secrets,
    xenon_endpoint: String,
    xenon_schedulers: Arc<DashMap<String, Arc<RwLock<Scheduler>>>>,
) -> Result<Vec<(String, Event)>, JobError> {
    // Decode payload into a command message.
    debug!("Decoding cmd message...");
    let command = match Command::decode(payload) {
        Ok(callback) => callback,
        Err(reason)  => { return Err(JobError::CommandDecodeError{ key, err: reason }); }
    };
    let kind = match CommandKind::from_i32(command.kind) {
        Some(kind) => kind,
        None       => { return Err(JobError::IllegalCommandKind{ kind: command.kind }); }
    };

    // Ignore unkown commands, as we can't dispatch it.
    if kind == CommandKind::Unknown {
        warn!("Received UNKOWN command (key: {}); ignoring message", key);
        return Ok(vec![]);
    }

    info!("Received {} command (key: {}).", kind, key);
    debug!("{:?}", command);

    // Dispatch command message to appropriate handlers.
    match kind {
        CommandKind::Create => {
            debug!("Handling CREATE command...");
            cmd_create::handle(&key, command, infra, secrets, xenon_endpoint, xenon_schedulers).await
        }
        CommandKind::Stop => unimplemented!(),
        CommandKind::Unknown => unreachable!(),
    }
}
/*******/
