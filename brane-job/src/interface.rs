use prost::{Enumeration, Message};
use serde::{Deserialize, Serialize};
use std::fmt;
use time::OffsetDateTime;

// #[derive(Clone, PartialEq, Message)]
// pub struct Callback {
//     #[prost(tag = "1", enumeration = "CallbackKind")]
//     pub kind: i32,
//     #[prost(tag = "2", string)]
//     pub job: String,
//     #[prost(tag = "3", string)]
//     pub application: String,
//     #[prost(tag = "4", string)]
//     pub location: String,
//     #[prost(tag = "5", int32)]
//     pub order: i32,
//     #[prost(tag = "6", bytes)]
//     pub payload: Vec<u8>,
// }

// impl Callback {
//     ///
//     ///
//     ///
//     pub fn new<S, B>(
//         kind: CallbackKind,
//         job: S,
//         application: S,
//         location: S,
//         order: i32,
//         payload: B,
//     ) -> Self
//     where
//         S: Into<String> + Clone,
//         B: Into<Vec<u8>>,
//     {
//         Callback {
//             kind: kind.into(),
//             job: job.into(),
//             application: application.into(),
//             location: location.into(),
//             order,
//             payload: payload.into(),
//         }
//     }
// }

// #[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
// pub enum CallbackKind {
//     Unknown = 0,
//     Ready = 1,
//     Initialized = 2,
//     Started = 3,
//     Heartbeat = 4,
//     Finished = 5,
//     Stopped = 6,
//     Failed = 7,
// }

// impl fmt::Display for CallbackKind {
//     fn fmt(
//         &self,
//         f: &mut fmt::Formatter<'_>,
//     ) -> fmt::Result {
//         write!(f, "{}", format!("{:?}", self).to_uppercase())
//     }
// }

#[derive(Clone, PartialEq, Message)]
pub struct Command {
    #[prost(tag = "1", enumeration = "CommandKind")]
    pub kind: i32,
    #[prost(tag = "2", optional, string)]
    pub identifier: Option<String>,
    #[prost(tag = "3", optional, string)]
    pub application: Option<String>,
    #[prost(tag = "4", optional, string)]
    pub location: Option<String>,
    #[prost(tag = "5", optional, string)]
    pub image: Option<String>,
    #[prost(tag = "6", repeated, string)]
    pub command: Vec<String>,
    #[prost(tag = "7", repeated, message)]
    pub mounts: Vec<Mount>,
}

impl Command {
    pub fn new<S: Into<String> + Clone>(
        kind: CommandKind,
        identifier: Option<S>,
        application: Option<S>,
        location: Option<S>,
        image: Option<S>,
        command: Vec<S>,
        mounts: Option<Vec<Mount>>,
    ) -> Self {
        Command {
            kind: kind as i32,
            identifier: identifier.map(S::into),
            application: application.map(S::into),
            location: location.map(S::into),
            image: image.map(S::into),
            command: command.iter().map(S::clone).map(S::into).collect(),
            mounts: mounts.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
pub enum CommandKind {
    Unknown = 0,
    Create = 1,
    Stop = 3,
}

impl fmt::Display for CommandKind {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_uppercase())
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct Event {
    #[prost(tag = "1", enumeration = "EventKind")]
    pub kind: i32,
    #[prost(tag = "2", string)]
    pub identifier: String,
    #[prost(tag = "3", string)]
    pub application: String,
    #[prost(tag = "4", string)]
    pub location: String,
    #[prost(tag = "5", string)]
    pub category: String,
    #[prost(tag = "6", uint32)]
    pub order: u32,
    #[prost(tag = "7", bytes)]
    pub payload: Vec<u8>,
    #[prost(tag = "8", int64)]
    pub timestamp: i64,
}

impl Event {
    ///
    ///
    ///
    #[allow(clippy::too_many_arguments)]
    pub fn new<S: Into<String> + Clone>(
        kind: EventKind,
        identifier: S,
        application: S,
        location: S,
        category: S,
        order: u32,
        payload: Option<Vec<u8>>,
        timestamp: Option<i64>,
    ) -> Self {
        let timestamp = timestamp.unwrap_or_else(|| OffsetDateTime::now_utc().unix_timestamp());

        Event {
            kind: kind as i32,
            identifier: identifier.into(),
            application: application.into(),
            location: location.into(),
            category: category.into(),
            order,
            payload: payload.unwrap_or_default(),
            timestamp,
        }
    }
}

/* TIM */
/// **Edited: added extra events to better monitor what's happening inside the branelet, changed numbers.**
/// 
/// Defines the possible events that can be sent between brane-job and brane-drv.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
pub enum EventKind {
    // Meta events
    /// Meta event for undefined states
    Unknown = 0,

    // Creation events
    /// We successfully created a container for the call
    Created      =  1,
    /// We could not create the container to run the call
    CreateFailed = -1,

    // Initialization events
    /// The container is ready with setting up the branelet executable (first opportunity for branelet to send events)
    Ready            =  2,
    /// The container has initialized its working directory
    Initialized      =  3,
    /// Something went wrong while setting up the working directory
    InitializeFailed = -3,
    /// The actual subcall executeable / script has started
    Started          =  4,
    /// The subprocess executable did not want to start (calling it failed)
    StartFailed      = -4,

    // Progress events
    /// Occassional message to let the user know the container is alive and running
    Heartbeat      =  5,
    /// The package call went wrong from the branelet's side
    CompleteFailed = -6,
    /// The package call went successfully from the branelet's side
    Completed      =  6,

    // Finish events
    /// brane-let could not decode the output from the package call
    DecodeFailed =  -8,
    /// The container has exited with a non-zero status code
    Failed       = -10,
    /// The container was interrupted by the Job node
    Stopped      =   9,
    /// The container has exited with a zero status code
    Finished     =  10,

    // Connection events (?)
    /// Something has connected (?)
    Connected    = 11,
    /// Something has disconnected (?)
    Disconnected = 12,
}

impl fmt::Display for EventKind {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_uppercase())
    }
}
/*******/



/// Defines the struct that will be used to transfer a failure result to the Driver
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureResult {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}



#[derive(Clone, PartialEq, Message)]
pub struct Mount {
    #[prost(tag = "1", string)]
    pub source: String,
    #[prost(tag = "2", string)]
    pub destination: String,
}

impl Mount {
    pub fn new<S: Into<String>>(
        source: S,
        destination: S,
    ) -> Self {
        Mount {
            source: source.into(),
            destination: destination.into(),
        }
    }
}
