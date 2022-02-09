use crate::errors::JobError;
use crate::interface::{Callback, Event};
use anyhow::Result;


/* TIM */
/// **Edited: added doc comments and now returning a JobError.**
/// 
/// Handles a Heartbeat callback message.
/// 
/// **Arguments**
///  * `callback`: The callback message we received, already parsed into a struct.
/// 
/// **Returns**  
/// A list of events to fire on success, or else a JobError listing what went wrong.
pub fn handle(callback: Callback) -> Result<Vec<(String, Event)>, JobError> {
    debug!("Received heartbeat callback: {:?}", callback);
    Ok(vec![])
}
/*******/
