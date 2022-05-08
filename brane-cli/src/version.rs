/* VERSION.rs
 *   by Lut99
 *
 * Created:
 *   08 May 2022, 13:31:16
 * Last edited:
 *   08 May 2022, 14:37:46
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Implements version queriers for the Brane framework.
**/

use std::fmt::{Display, Formatter, Result as FResult};
use std::str::FromStr;

use log::debug;
use reqwest::{Response, StatusCode};

use specifications::registry::RegistryConfig;
use specifications::version::Version;

use crate::errors::VersionError;
use crate::utils::get_config_dir;


/***** HELPER STRUCTS *****/
/// Struct that is used in querying the local CLI.
#[derive(Debug)]
struct LocalVersion {
    /// The version as reported by the env
    version : Version,
}

impl LocalVersion {
    /// Constructor for the RemoteVersion.
    /// 
    /// Queries the CARGO_PKG_VERSION environment variable for the version.
    /// 
    /// # Returns
    /// A new LocalVersion instance on success, or else a VersionError.
    fn new() -> Result<Self, VersionError> {
        // Parse the env first
        let version = match Version::from_str(env!("CARGO_PKG_VERSION")) {
            Ok(version) => version,
            Err(err)    => { return Err(VersionError::VersionParseError{ raw: env!("CARGO_PKG_VERSION").to_string(), err }); }
        };

        // Done, return the struct
        Ok(Self {
            version,
        })
    }
    
}

impl Display for LocalVersion {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "{}", self.version)
    }
}



/// Struct that is used in querying the remote CLI.
#[derive(Debug)]
struct RemoteVersion {
    /// The version as downloaded from the remote
    version : Version,
}

impl RemoteVersion {
    /// Constructor for the RemoteVersion.
    /// 
    /// Queries the remote host as stored in the Brane registry login file (get_config_dir()/registry.yml) for its version number.
    /// 
    /// # Returns
    /// A new RemoteVersion instance on success, or else a VersionError.
    async fn new() -> Result<Self, VersionError> {
        debug!("Retrieving remote version number");

        // Try to get the registry file path
        debug!(" > Reading registy.yml...");
        let config_file = match get_config_dir() {
            Ok(dir)  => dir.join("registry.yml"),
            Err(err) => { return Err(VersionError::ConfigDirError{ err }); }
        };

        // We are, so load the registry file
        let registry = match RegistryConfig::from_path(&config_file) {
            Ok(registry) => registry,
            Err(err)     => { return Err(VersionError::RegistryFileError{ err }); }
        };

        // Pass to the other constructor
        Self::from_registry_file(registry).await
    }

    /// Constructor for the RemoteVersion, which creates it from a given RegistryConfig.
    /// 
    /// # Arguments
    /// - `registry`: The RegistryConfig file to use to find the remote registry properties.
    /// 
    /// # Returns
    /// A new RemoteVersion instance on success, or else a VersionError.
    async fn from_registry_file(registry: RegistryConfig) -> Result<Self, VersionError> {
        // Use reqwest for the API call
        debug!(" > Querying...");
        let mut url: String = registry.url.clone(); url.push_str("/version");
        let response: Response = match reqwest::get(&url).await {
            Ok(version) => version,
            Err(err)    => { return Err(VersionError::RequestError{ url, err }); }
        };
        if response.status() != StatusCode::OK {
            return Err(VersionError::RequestFailure{ url, status: response.status() });
        }
        let version_body: String = match response.text().await {
            Ok(body) => body,
            Err(err) => { return Err(VersionError::RequestBodyError{ url, err }); }
        };

        // Try to parse the version
        debug!(" > Parsing remote version...");
        let version = match Version::from_str(&version_body) {
            Ok(version) => version,
            Err(err)    => { return Err(VersionError::VersionParseError{ raw: version_body, err }); }  
        };

        // Done!
        debug!("Remote version number: {}", &version);
        Ok(Self {
            version,
        })
    }
}

impl Display for RemoteVersion {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "{}", self.version)
    }
}





/***** HANDLERS *****/
/// Returns the local version (without any extra text).
pub fn handle_local() -> Result<(), VersionError> {
    // Get the local version and print it
    println!("v{}", LocalVersion::new()?);

    // Done
    Ok(())
}



/// Returns the local version (without any extra text).
pub async fn handle_remote() -> Result<(), VersionError> {
    // Get the remote version and print it
    println!("v{}", RemoteVersion::new().await?);

    // Done
    Ok(())
}



/// Returns both the local and possible remote version numbers with some pretty formatting.
pub async fn handle() -> Result<(), VersionError> {
    // Get the local version first and immediately print
    println!();
    println!("Brane CLI client");
    println!(" - Version: v{}", LocalVersion::new()?);
    println!();

    // If the registry file exists, then also do the remote
    let config_file = match get_config_dir() {
        Ok(dir)  => dir.join("registry.yml"),
        Err(err) => { return Err(VersionError::ConfigDirError{ err }); }
    };
    if config_file.exists() {
        // Get the registry file from it
        let registry = match RegistryConfig::from_path(&config_file) {
            Ok(registry) => registry,
            Err(err)     => { return Err(VersionError::RegistryFileError{ err }); }
        };

        // Print the URL
        println!("Remote Brane instance at '{}'", &registry.url);
        
        // Get the version
        let version = RemoteVersion::from_registry_file(registry).await?;
        println!(" - Version: v{}", version);
        println!();
    }

    // Done
    Ok(())
}
