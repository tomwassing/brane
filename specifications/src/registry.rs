/* REGISTRY.rs
 *   by Lut99
 *
 * Created:
 *   08 May 2022, 13:57:01
 * Last edited:
 *   08 May 2022, 14:07:07
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains file and other struct definitions for registry-related stuff
 *   (brane-cli only).
**/

use std::error::Error;
use std::io::ErrorKind;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;


/***** ERRORS *****/
/// Defines possible errors when loading a RegistryConfig file.
#[derive(Debug)]
pub enum RegistryConfigError {
    /// The registry file was not found (i.e., not logged in).
    NotLoggedIn{ path: PathBuf },
    /// Could not open the given file.
    FileOpenError{ path: PathBuf, err: std::io::Error },
    /// Could not parse the given file.
    FileParseError{ path: PathBuf, err: serde_yaml::Error },
}

impl Display for RegistryConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use RegistryConfigError::*;
        match self {
            NotLoggedIn{ path }         => write!(f, "You are not logged in; run the 'login' subcommand first (or registry file '{}' is missing)", path.display()),
            FileOpenError{ path, err }  => write!(f, "Could not open registry file '{}': {}", path.display(), err),
            FileParseError{ path, err } => write!(f, "Could not parse registry file '{}': {}", path.display(), err),
        }
    }
}

impl Error for RegistryConfigError {}





/***** LIBRARY *****/
/// File that represents the registry to which this brane-cli instance is connected.
#[skip_serializing_none]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryConfig {
    /// The endpoint of the remote registry.
    pub url: String,
    /// The username with which we sign packages.
    pub username: String,
}

impl RegistryConfig {
    /// Constructor for the RegistryConfig, which loads it from the given file.
    /// 
    /// # Arguments
    /// - `path`: The Path to the file to load.
    /// 
    /// # Returns
    /// A new RegistryConfig on success, or else a RegistryConfigError.
    pub fn from_path(path: &Path) -> Result<RegistryConfig, RegistryConfigError> {
        // Try to open the given file
        let handle = match File::open(path) {
            Ok(handle) => handle,
            Err(err)   => {
                // Do different errors depending on the kind
                return match err.kind() {
                    ErrorKind::NotFound => Err(RegistryConfigError::NotLoggedIn{ path: path.to_path_buf() }),
                    _                   => Err(RegistryConfigError::FileOpenError{ path: path.to_path_buf(), err }),
                };
            }
        };

        // Try to parse it with serde; done
        match serde_yaml::from_reader(handle) {
            Ok(result) => Ok(result),
            Err(err)   => Err(RegistryConfigError::FileParseError{ path: path.to_path_buf(), err }),
        }
    }
}
