/* ERRORS.rs
 *   by Lut99
 *
 * Created:
 *   01 Feb 2022, 16:13:53
 * Last edited:
 *   01 Feb 2022, 17:07:40
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains errors used within the brane-drv package only.
**/

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use rdkafka::error::KafkaError;
use rdkafka::error::RDKafkaErrorCode;


/***** ERRORS *****/
/// Errors that occur during the main phase of the brane-drv package
#[derive(Debug)]
pub enum DriverError {
    /// Could not create a Kafka client
    KafkaClientError{ servers: String, err: KafkaError },
    /// Could not get the Kafka client to try to add more topics
    KafkaTopicsError{ topics: String, err: KafkaError },
    /// Could not add the given topic (with a duplicate error already filtered out)
    KafkaTopicError{ topic: String, err: RDKafkaErrorCode },
}

impl DriverError {
    /// Serializes a given list of vectors into a string.
    /// 
    /// **Generic types**
    ///  * `T`: The type of the vector. Must be convertible to string via the Display trait.
    /// 
    /// **Arguments**
    ///  * `v`: The Vec to serialize.
    /// 
    /// **Returns**  
    /// A string describing the vector. Nothing too fancy, just a list separated by commas.
    pub fn serialize_vec<T>(v: &Vec<T>) -> String
    where
        T: Display
    {
        let mut res: String = String::new();
        for e in v {
            if res.len() == 0 { res += ", "; }
            res += &format!("'{}'", e);
        }
        res
    }
}

impl Display for DriverError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            DriverError::KafkaClientError{ servers, err } => write!(f, "Could not create Kafka client with bootstrap servers '{}': {}", servers, err),
            DriverError::KafkaTopicsError{ topics, err }  => write!(f, "Could not create new Kafka topics '{}': {}", topics, err),
            DriverError::KafkaTopicError{ topic, err }    => write!(f, "Coult not create Kafka topic '{}': {}", topic, err),
        }
    }
}

impl Error for DriverError {}
