use crate::common::{CallPattern, Parameter, Type};
use crate::package::PackageKind;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs;
use std::io::Read;
use std::path::PathBuf;


/***** CUSTOM TYPES *****/
type Map<T> = std::collections::HashMap<String, T>;





/***** ERRORS *****/
/// Collects errors relating to the Container specification.
#[derive(Debug)]
pub enum ContainerInfoError {
    /// Error for when a file couldn't be read
    IOReadError{ path: PathBuf, err: std::io::Error },
    /// Could not parse the given file with YAML
    YAMLParseError{ err: serde_yaml::Error },
}

impl Display for ContainerInfoError {
    fn fmt (&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            ContainerInfoError::IOReadError{ path, err } => write!(f, "Could not read container information from file '{}': {}", path.display(), err),
            ContainerInfoError::YAMLParseError{ err }    => write!(f, "Could not parse container information from a string with YAML contents: {}", err),
        }
    }
}

impl Error for ContainerInfoError {}





/***** SPECIFICATIONS *****/
/// Specifies the contents of a container info YAML file.
#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub actions: Map<Action>,
    pub base: Option<String>,
    pub contributors: Option<Vec<String>>,
    pub description: Option<String>,
    pub entrypoint: Entrypoint,
    pub environment: Option<Map<String>>,
    pub dependencies: Option<Vec<String>>,
    pub files: Option<Vec<String>>,
    pub initialize: Option<Vec<String>>,
    pub install: Option<Vec<String>>,
    pub kind: PackageKind,
    pub name: String,
    pub types: Option<Map<Type>>,
    pub version: String,
}

#[allow(unused)]
impl ContainerInfo {
    /// **Edited: now returning ContainerInfoErrors.**
    /// 
    /// Returns a ContainerInfo by constructing it from the file at the given path.
    /// 
    /// **Arguments**
    ///  * `path`: The path to the container info file.
    /// 
    /// **Returns**  
    /// The newly constructed ContainerInfo instance on success, or a ContainerInfoError upon failure.
    pub fn from_path(path: PathBuf) -> Result<ContainerInfo, ContainerInfoError> {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err)     => { return Err(ContainerInfoError::IOReadError{ path, err }); }
        };

        // Delegate the actual parsing to from_string
        ContainerInfo::from_string(contents)
    }

    /// **Edited: now returning ContainerInfoErrors.**
    /// 
    /// Returns a ContainerInfo by constructing it from the given Reader with YAML text.
    /// 
    /// **Arguments**
    ///  * `r`: The reader with the contents of the raw YAML file.
    /// 
    /// **Returns**  
    /// The newly constructed ContainerInfo instance on success, or a ContainerInfoError upon failure.
    pub fn from_reader<R: Read>(r: R) -> Result<ContainerInfo, ContainerInfoError> {
        match serde_yaml::from_reader(r) {
            Ok(result) => Ok(result),
            Err(err)   => Err(ContainerInfoError::YAMLParseError{ err }),
        }
    }

    /// **Edited: now returning ContainerInfoErrors.**
    /// 
    /// Returns a ContainerInfo by constructing it from the given string of YAML text.
    /// 
    /// **Arguments**
    ///  * `contents`: The text contents of a raw YAML file.
    /// 
    /// **Returns**  
    /// The newly constructed ContainerInfo instance on success, or a ContainerInfoError upon failure.
    pub fn from_string(contents: String) -> Result<ContainerInfo, ContainerInfoError> {
        match serde_yaml::from_str(&contents) {
            Ok(result) => Ok(result),
            Err(err)   => Err(ContainerInfoError::YAMLParseError{ err }),
        }
    }
}



/// Defines the YAML of an action in a package.
#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub command: Option<ActionCommand>,
    pub description: Option<String>,
    pub endpoint: Option<ActionEndpoint>,
    pub pattern: Option<CallPattern>,
    pub input: Option<Vec<Parameter>>,
    pub output: Option<Vec<Parameter>>,
}



/// Defines the YAML of a command within an action in a package.
#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionCommand {
    pub args: Vec<String>,
    pub capture: Option<String>,
}



/// Defines the YAML of a remote OAS action.
#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEndpoint {
    pub method: Option<String>,
    pub path: String,
}



/// Defines the YAML of the entry point to a package (in terms of function).
#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entrypoint {
    pub kind: String,
    pub exec: String,
    pub content: Option<String>,
    pub delay: Option<u64>,
}
