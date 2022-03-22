use crate::Secrets;
use crate::store::{Store, StoreError};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;


/* TIM */
/***** ERRORS *****/
/// Lists errors that can occur while working with infrastructure files
#[derive(Debug)]
pub enum InfrastructureError {
    /// Could not canonicalize the given Store path
    StoreError{ err: StoreError },
    /// Could not open the local database file
    LocalOpenError{ path: PathBuf, err: std::io::Error },
    /// Could not read the local database file
    LocalIOError{ path: PathBuf, err: std::io::Error },

    /// Encountered an empty infra.yml
    EmptyInfraFile{ path: PathBuf },
    /// The given infra.yml cannot be read as YML
    InvalidInfraFile{ path: PathBuf, err: serde_yaml::Error },
    /// The given location does not appear in the infrastructure file
    UnknownLocation{ location: String },

    /// The Database functionality of a remote infrastructure file isn't implemented yet
    DatabaseNotImplemented,
}

impl std::fmt::Display for InfrastructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfrastructureError::StoreError{ err }           => write!(f, "Could not resolve infrastructure file location: {}", err),
            InfrastructureError::LocalOpenError{ path, err } => write!(f, "Could not open local infrastructure file '{}': {}", path.display(), err),
            InfrastructureError::LocalIOError{ path, err }   => write!(f, "Could not read local infrastructure file '{}': {}", path.display(), err),

            InfrastructureError::EmptyInfraFile{ path }        => write!(f, "Infrastructure file '{}' is empty", path.display()),
            InfrastructureError::InvalidInfraFile{ path, err } => write!(f, "Invalid infrastructure file '{}': {}", path.display(), err),
            InfrastructureError::UnknownLocation{ location }   => write!(f, "Unknown location identifier '{}'", location),

            InfrastructureError::DatabaseNotImplemented => write!(f, "Storing infra.yml in a remote database is not yet implemented"),
        }
    }
}

impl std::error::Error for InfrastructureError {}
/*******/


#[derive(Clone, Debug, Deserialize, Default)]
pub struct InfrastructureDocument {
    locations: HashMap<String, Location>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Location {
    Kube {
        address: String,
        callback_to: String,
        namespace: String,
        registry: String,
        credentials: LocationCredentials,
        proxy_address: Option<String>,
        mount_dfs: Option<String>,
    },
    Local {
        address: Option<String>,
        callback_to: String,
        network: String,
        registry: String,
        proxy_address: Option<String>,
        mount_dfs: Option<String>,
    },
    Vm {
        address: String,
        callback_to: String,
        runtime: String,
        registry: String,
        credentials: LocationCredentials,
        proxy_address: Option<String>,
        mount_dfs: Option<String>,
    },
    Slurm {
        address: String,
        callback_to: String,
        runtime: String,
        registry: String,
        credentials: LocationCredentials,
        proxy_address: Option<String>,
        mount_dfs: Option<String>,
    },
}

impl Location {
    pub fn get_address(&self) -> String {
        match self {
            Location::Kube { address, .. } | Location::Vm { address, .. } | Location::Slurm { address, .. } => {
                address.clone()
            }
            Location::Local { address, .. } => address.clone().unwrap_or_else(|| String::from("127.0.0.1")),
        }
    }

    pub fn get_registry(&self) -> String {
        match self {
            Location::Kube { registry, .. }
            | Location::Vm { registry, .. }
            | Location::Slurm { registry, .. }
            | Location::Local { registry, .. } => registry.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mechanism", rename_all = "kebab-case")]
pub enum LocationCredentials {
    Config {
        file: String,
    },
    SshCertificate {
        username: String,
        certificate: String,
        passphrase: Option<String>,
    },
    SshPassword {
        username: String,
        password: String,
    },
}

impl LocationCredentials {
    ///
    ///
    ///
    pub fn resolve_secrets(
        &self,
        secrets: &Secrets,
    ) -> Self {
        use LocationCredentials::*;

        let resolve = |value: &String| {
            // Try to resolve secret, but use the value as-is otherwise.
            if let Some(value) = value.strip_prefix("s$") {
                if let Ok(secret) = secrets.get(value) {
                    return secret;
                }
            }

            value.clone()
        };

        match self {
            Config { file } => {
                let file = resolve(file);

                Config { file }
            }
            SshCertificate {
                username,
                certificate,
                passphrase,
            } => {
                let username = resolve(username);
                let certificate = resolve(certificate);
                let passphrase = passphrase.clone().map(|p| resolve(&p));

                SshCertificate {
                    username,
                    certificate,
                    passphrase,
                }
            }
            SshPassword { username, password } => {
                let username = resolve(username);
                let password = resolve(password);

                SshPassword { username, password }
            }
        }
    }

    /* TIM */
    /// Returns a human-readable name of the credential type.
    #[inline]
    pub fn cred_type(&self) -> &str {
        match self {
            LocationCredentials::Config{ .. }         => "Config",
            LocationCredentials::SshCertificate{ .. } => "SshCertificate",
            LocationCredentials::SshPassword{ .. }    => "SshPassword",
        }
    }
    /*******/
}

#[derive(Clone, Debug)]
pub struct Infrastructure {
    store: Store,
}

impl Infrastructure {
    /* TIM */
    /// **Edited: Now returning InfrastructureErrors.**
    ///
    /// Constructor for the Infrastructure.
    /// 
    /// **Arguments**
    ///  * `store`: The location of the infrastructure file, which can either be a remote location (via an URL) or a local file (via a path).
    /// 
    /// **Returns**  
    /// A new instance of an Infrastructure on success or an InfrastructureError otherwise.
    pub fn new<S: Into<String>>(store: S) -> Result<Self, InfrastructureError> {
        // Convert the string-like to a string
        let store = store.into();

        // Try to convert to a proper Store
        match Store::from(store) {
            Ok(store)   => Ok(Infrastructure{ store }),
            Err(reason) => Err(InfrastructureError::StoreError{ err: reason }),
        }
    }
    /*******/

    /* TIM */
    /// Helper function that opens, reads and parses an infra.yml file.
    /// 
    /// **Arguments**
    ///  * `store`: The Store describing where the file is located.
    /// 
    /// **Returns**  
    /// The file's contents as an InfrastructureDocument on success, or a description of the failure as an InfrastructureError.
    fn read_store(store: &Store) -> Result<InfrastructureDocument, InfrastructureError> {
        if let Store::File(store_file) = store {
            // Open a handle to the local file
            let infra_handle = match File::open(store_file) {
                Ok(infra_handle) => infra_handle,
                Err(reason)      => { return Err(InfrastructureError::LocalOpenError{ path: store_file.clone(), err: reason }); }
            };
            let mut infra_reader = BufReader::new(infra_handle);

            // Read it into memory in one go
            let mut infra_file = String::new();
            if let Err(reason) = infra_reader.read_to_string(&mut infra_file) { return Err(InfrastructureError::LocalIOError{ path: store_file.clone(), err: reason }); }
            if infra_file.is_empty() { return Err(InfrastructureError::EmptyInfraFile{ path: store_file.clone() }); }

            // Finally, try to parse using serde
            match serde_yaml::from_str::<InfrastructureDocument>(&infra_file) {
                Ok(result)  => Ok(result),
                Err(reason) => Err(InfrastructureError::InvalidInfraFile{ path: store_file.clone(), err: reason }),
            }
        } else {
            // We didn't program this path yet
            Err(InfrastructureError::DatabaseNotImplemented)
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Now returning InfrastructureErrors.**
    /// 
    /// Validates the Infrastructure file.  
    /// Note that this function is slow, as the infrastructure file isn't actually read from memory.
    /// 
    /// **Returns**  
    /// Nothing if the file was valid, or an InfrastructureError detailling why it wasn't otherwise.
    pub fn validate(&self) -> Result<(), InfrastructureError> {
        // Simply check if we can read it without any problems
        match Self::read_store(&self.store) {
            Ok(_)       => Ok(()),
            Err(reason) => Err(reason),
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Now returning InfrastructureErrors.**
    ///
    /// Returns the list of location names in the infra.yml.
    /// 
    /// **Returns**  
    /// The locations as a vector of string identifiers, or an InfrastructureError if we failed to do so.
    pub fn get_locations(&self) -> Result<Vec<String>, InfrastructureError> {
        // Read the infrastructure file
        let infra_document = Self::read_store(&self.store)?;

        // Return the locations, easily mapped
        Ok(infra_document.locations.keys().map(|k| k.to_string()).collect())
    }
    /*******/

    /* TIM */
    /// **Edited: Now returning InfrastructureErrors.**
    /// 
    /// Returns the metadata (=data) of the given location.
    /// 
    /// **Arguments**
    ///  * `location`: The string(-like) identifier of the location to read the metadata from.
    /// 
    /// **Returns**  
    /// The location as a Location object on success, or an InfrastructureError describing what went wrong otherwise.
    pub fn get_location_metadata<S: Into<String>>(
        &self,
        location: S,
    ) -> Result<Location, InfrastructureError> {
        // Convert the string-like into a string
        let location = location.into();

        // Read the file
        let infra_document = Self::read_store(&self.store)?;

        // Return the location
        match infra_document.locations.get(&location) {
            Some(location) => Ok(location.clone()),
            None           => Err(InfrastructureError::UnknownLocation{ location }),
        }
    }
    /*******/
}
