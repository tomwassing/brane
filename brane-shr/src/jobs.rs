#[repr(u8)]
/* TIM */
// #[derive(Debug, PartialEq, PartialOrd, FromPrimitive, ToPrimitive)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, FromPrimitive, ToPrimitive)]
/*******/
pub enum JobStatus {
    Unknown = 0,
    Created = 1,
    Ready = 2,
    Initialized = 3,
    Started = 4,
    Finished = 5,
    Stopped = 6,
    Failed = 7,
}
