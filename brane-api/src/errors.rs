/* ERRORS.rs
 *   by Lut99
 *
 * Created:
 *   04 Feb 2022, 10:35:12
 * Last edited:
 *   04 Feb 2022, 10:42:19
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains general errors for across the brane-api package.
**/

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};

use scylla::transport::errors::NewSessionError;


/***** ERRORS *****/
/// Collects errors for the most general case in the brane-api package
#[derive(Debug)]
pub enum ApiError {
    /// Could not create a Scylla session
    ScyllaConnectError{ host: String, err: NewSessionError },
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            ApiError::ScyllaConnectError{ host, err } => write!(f, "Could not connect to Scylla host '{}': {}", host, err),
        }
    }
}

impl Error for ApiError {}
