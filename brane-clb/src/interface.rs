use prost::{Enumeration, Message};
use std::fmt::{Display, Formatter, Result as FResult};

#[derive(Clone, PartialEq, Message)]
pub struct Callback {
    #[prost(tag = "1", enumeration = "CallbackKind")]
    pub kind: i32,
    #[prost(tag = "2", string)]
    pub job: String,
    #[prost(tag = "3", string)]
    pub application: String,
    #[prost(tag = "4", string)]
    pub location: String,
    #[prost(tag = "5", int32)]
    pub order: i32,
    #[prost(tag = "6", bytes)]
    pub payload: Vec<u8>,
}

impl Callback {
    ///
    ///
    ///
    pub fn new<S, B>(
        kind: CallbackKind,
        job: S,
        application: S,
        location: S,
        order: i32,
        payload: B,
    ) -> Self
    where
        S: Into<String> + Clone,
        B: Into<Vec<u8>>,
    {
        Callback {
            kind: kind.into(),
            job: job.into(),
            application: application.into(),
            location: location.into(),
            order,
            payload: payload.into(),
        }
    }
}

/// **Edited: adding failure states.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
pub enum CallbackKind {
    Unknown = 0,

    Ready = 1,

    InitializeFailed = 2,
    Initialized = 3,

    StartFailed = 4,
    Started = 5,

    Heartbeat = 6,
    CompleteFailed = 7,
    Completed = 8,

    DecodeFailed = 9,
    Stopped = 10,
    Failed = 11,
    Finished = 12,
}

impl Display for CallbackKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "{}", format!("{:?}", self).to_uppercase())
    }
}
