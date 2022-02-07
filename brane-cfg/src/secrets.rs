use crate::store::{Store, StoreError};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;


/* TIM */
/***** ERRORS *****/
/// Lists errors that can occur while working with secrets files
#[derive(Debug)]
pub enum SecretsError {
    /// Could not canonicalize the given Store path
    StoreError{ err: StoreError },
    /// Could not open the local database file
    LocalOpenError{ path: PathBuf, err: std::io::Error },
    /// Could not read the local database file
    LocalIOError{ path: PathBuf, err: std::io::Error },

    /// Encountered an empty secrets.yml
    EmptySecretsFile{ path: PathBuf },
    /// The given secrets.yml cannot be read as YML
    InvalidSecretsFile{ path: PathBuf, err: serde_yaml::Error },
    /// The given secret does not appear in the secrets file
    UnknownSecret{ secret: String },

    /// The Database functionality of a remote secrets file isn't implemented yet
    DatabaseNotImplemented,
}

impl std::fmt::Display for SecretsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretsError::StoreError{ err }           => write!(f, "Could not resolve secrets file location: {}", err),
            SecretsError::LocalOpenError{ path, err } => write!(f, "Could not open local secrets file '{}': {}", path.display(), err),
            SecretsError::LocalIOError{ path, err }   => write!(f, "Could not read local secrets file '{}': {}", path.display(), err),

            SecretsError::EmptySecretsFile{ path }        => write!(f, "Secrets file '{}' is empty", path.display()),
            SecretsError::InvalidSecretsFile{ path, err } => write!(f, "Invalid secrets file '{}': {}", path.display(), err),
            SecretsError::UnknownSecret{ secret }         => write!(f, "Unknown secret identifier '{}'", secret),

            SecretsError::DatabaseNotImplemented => write!(f, "Storing secrets.yml in a remote database is not yet implemented"),
        }
    }
}

impl std::error::Error for SecretsError {}
/*******/


/// Defines the internal representation of a Secrets document
pub type SecretsDocument = HashMap<String, String>;


#[derive(Clone, Debug)]
pub struct Secrets {
    store: Store,
}

impl Secrets {
    /* TIM */
    /// **Edited: Now returning SecretsError.**
    ///
    /// Constructor for the Secrets.
    /// 
    /// **Arguments**
    ///  * `store`: The location of the secrets file, which can either be a remote location (via an URL) or a local file (via a path).
    /// 
    /// **Returns**  
    /// A new instance of a Secrets on success or an SecretsError otherwise.
    pub fn new<S: Into<String>>(store: S) -> Result<Self, SecretsError> {
        match Store::from(store) {
            Ok(store)   => Ok(Secrets{ store }),
            Err(reason) => Err(SecretsError::StoreError{ err: reason }),
        }
    }
    /*******/

    /* TIM */
    /// Helper function that opens, reads and parses a secrets.yml file.
    /// 
    /// **Arguments**
    ///  * `store`: The Store describing where the file is located.
    /// 
    /// **Returns**  
    /// The file's contents as a map of secrets on success, or a description of the failure as a SecretsError.
    fn read_store(store: &Store) -> Result<SecretsDocument, SecretsError> {
        if let Store::File(store_file) = store {
            // Open a handle to the local file
            let infra_handle = match File::open(store_file) {
                Ok(infra_handle) => infra_handle,
                Err(reason)      => { return Err(SecretsError::LocalOpenError{ path: store_file.clone(), err: reason }); }
            };
            let mut infra_reader = BufReader::new(infra_handle);

            // Read it into memory in one go
            let mut infra_file = String::new();
            if let Err(reason) = infra_reader.read_to_string(&mut infra_file) { return Err(SecretsError::LocalIOError{ path: store_file.clone(), err: reason }); }
            if infra_file.is_empty() { return Err(SecretsError::EmptySecretsFile{ path: store_file.clone() }); }

            // Finally, try to parse using serde
            match serde_yaml::from_str::<SecretsDocument>(&infra_file) {
                Ok(result)  => Ok(result),
                Err(reason) => Err(SecretsError::InvalidSecretsFile{ path: store_file.clone(), err: reason }),
            }
        } else {
            // We didn't program this path yet
            Err(SecretsError::DatabaseNotImplemented)
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Now returning SecretsErrors.**
    /// 
    /// Validates the Secrets file.  
    /// Note that this function is slow, as the secrets file isn't actually read from memory.
    /// 
    /// **Returns**  
    /// Nothing if the file was valid, or a SecretsError detailling why it wasn't otherwise.
    pub fn validate(&self) -> Result<(), SecretsError> {
        // Simply check if we can read it without any problems
        match Self::read_store(&self.store) {
            Ok(_)       => Ok(()),
            Err(reason) => Err(reason),
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Now returning SecretsErrors.**
    /// 
    /// Returns the value of the given secret.
    /// 
    /// **Arguments**
    ///  * `secret_key`: The string(-like) identifier of the secret we want to retrieve.
    /// 
    /// **Returns**  
    /// The secret's value as a String, or a SecretsError upon a failure.
    pub fn get<S: Into<String>>(
        &self,
        secret_key: S,
    ) -> Result<String, SecretsError> {
        // Convert the string-like to a string
        let secret_key = secret_key.into();

        // Read the secrets file
        let secrets_document = Self::read_store(&self.store)?;

        // Return the value
        match secrets_document.get(&secret_key) {
            Some(value) => Ok(value.clone()),
            None        => Err(SecretsError::UnknownSecret{ secret: secret_key }),
        }
    }
    /*******/
}
