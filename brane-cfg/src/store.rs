/* STORE.rs
 *   by Lut99
 *
 * Created:
 *   07 Feb 2022, 11:06:45
 * Last edited:
 *   07 Feb 2022, 11:32:41
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains the common-used Store object that defines either a local or a
 *   remote store of something.
**/

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs;
use std::path::PathBuf;

use url::Url;


/***** ERRORS *****/
/// Collects errors used by the Store object
#[derive(Debug)]
pub enum StoreError {
    /// Could not canonicalize the given path
    UncanonicalizeablePath{ path: String, err: std::io::Error },
}

impl Display for StoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            StoreError::UncanonicalizeablePath{ path, err } => write!(f, "Could not canonicalize the given path '{}': {}", path, err),
        }
    }
}

impl Error for StoreError {}





/***** THE STORE *****/
/// Defines a resource identifier for either a local file (File) or a remote file (Database)
#[derive(Clone, Debug)]
pub enum Store {
    File(PathBuf),
    Database(Url),
}

impl Store {
    /* TIM */
    /// **Edited: Now returning StoreErrors + moved to its own file.**
    /// 
    /// Tries to convert a given string into a Store object.
    /// 
    /// **Arguments**
    ///  * `store`: The string(-like) object to convert to a Store.
    /// 
    /// **Returns**  
    /// Either a remote Database or a File as a Store on success, or a StoreError on failure.
    pub fn from<S: Into<String>>(store: S) -> Result<Self, StoreError> {
        // Convert the store to a string
        let store = store.into();

        // Check if it's a URL or a path
        if let Ok(url) = Url::parse(&store) {
            Ok(Store::Database(url))
        } else {
            match fs::canonicalize(&store) {
                Ok(file_path) => Ok(Store::File(file_path)),
                Err(reason)   => Err(StoreError::UncanonicalizeablePath{ path: store.clone(), err: reason }),
            }
        }
    }
    /*******/
}
