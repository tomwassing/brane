use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::common::{CallPattern, Parameter, Type};
use crate::package::PackageKind;
use crate::version::Version;


/***** CUSTOM TYPES *****/
type Map<T> = std::collections::HashMap<String, T>;





/***** ERRORS *****/
/// Collect errors relating to the LocalContainer specification.
#[derive(Debug)]
pub enum LocalContainerInfoError {
    /// Could not open the target file
    FileOpenError{ path: PathBuf, err: std::io::Error },
    /// Could not parse the target file
    FileParseError{ err: serde_yaml::Error },

    /// Could not create the target file
    FileCreateError{ path: PathBuf, err: std::io::Error },
    /// Could not write to the given writer
    FileWriteError{ err: serde_yaml::Error },
}

impl Display for LocalContainerInfoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            LocalContainerInfoError::FileOpenError{ path, err } => write!(f, "Could not open local container file '{}': {}", path.display(), err),
            LocalContainerInfoError::FileParseError{ err }      => write!(f, "Could not read & parse local container file: {}", err),

            LocalContainerInfoError::FileCreateError{ path, err } => write!(f, "Could not create local container file '{}': {}", path.display(), err),
            LocalContainerInfoError::FileWriteError{ err }        => write!(f, "Could not serialize & write local container file: {}", err),
        }
    }
}

impl Error for LocalContainerInfoError {}



/// Collects errors relating to the Container specification.
#[derive(Debug)]
pub enum ContainerInfoError {
    /// Could not open the target file
    FileReadError{ path: PathBuf, err: std::io::Error },
    /// Could not parse the target file
    ParseError{ err: serde_yaml::Error },

    /// Could not create the target file
    FileCreateError{ path: PathBuf, err: std::io::Error },
    /// Could not write to the given writer
    FileWriteError{ err: serde_yaml::Error },
}

impl Display for ContainerInfoError {
    fn fmt (&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            ContainerInfoError::FileReadError{ path, err } => write!(f, "Could not open & read container file '{}': {}", path.display(), err),
            ContainerInfoError::ParseError{ err }          => write!(f, "Could not parse container file YAML: {}", err),

            ContainerInfoError::FileCreateError{ path, err } => write!(f, "Could not create container file '{}': {}", path.display(), err),
            ContainerInfoError::FileWriteError{ err }        => write!(f, "Could not serialize & write container file: {}", err),
        }
    }
}

impl Error for ContainerInfoError {}





/***** SPECIFICATIONS *****/
/// Specifies the contents of a contaienr info YAML file that is inside the container itself.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalContainerInfo {
    /// The name of the package
    pub name       : String,
    /// The kind of the package
    pub kind       : PackageKind,
    /// The entrypoint to the package
    pub entrypoint : Entrypoint,
    /// The list of actions that this package can do.
    pub actions    : Map<Action>,
    /// The list of types that are declared in this package.
    pub types      : Map<Type>,
}

impl LocalContainerInfo {
    /// Constructor for the LocalContainerInfo that constructs it from the given path.
    /// 
    /// **Generic types**
    ///  * `P`: The Path-like type of the path given.
    /// 
    /// **Arguments**
    ///  * `path`: the Path to read the new LocalContainerInfo from.
    /// 
    /// **Returns**  
    /// A new LocalContainerInfo on success, or else a LocalContainerInfoError.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, LocalContainerInfoError> {
        // Convert the path-like to a Path
        let path = path.as_ref();

        // Open the file to read it
        let handle = match File::open(path) {
            Ok(handle) => handle,
            Err(err)   => { return Err(LocalContainerInfoError::FileOpenError{ path: path.to_path_buf(), err }); }
        };

        // Do the parsing with from_reader()
        Self::from_reader(handle)
    }

    /// Constructor for the LocalContainerInfo that constructs it from the given reader.
    /// 
    /// **Generic types**
    ///  * `R`: The type of the reader, which implements Read.
    /// 
    /// **Arguments**
    ///  * `reader`: The reader to read from. Will be consumed.
    /// 
    /// **Returns**  
    /// A new LocalContainerInfo on success, or else a LocalContainerInfoError.
    pub fn from_reader<R: Read>(reader: R) -> Result<Self, LocalContainerInfoError> {
        // Simply pass to serde
        match serde_yaml::from_reader(reader) {
            Ok(result) => Ok(result),
            Err(err)   => Err(LocalContainerInfoError::FileParseError{ err }),
        }
    }



    /// Writes the LocalContainerInfo to the given location.
    /// 
    /// **Generic types**
    ///  * `P`: The Path-like type of the given target location.
    /// 
    /// **Arguments**
    ///  * `path`: The target location to write to the LocalContainerInfo to.
    /// 
    /// **Returns**  
    /// Nothing on success, or a LocalContainerInfoError otherwise.
    pub fn to_path<P: AsRef<Path>>(&self, path: P) -> Result<(), LocalContainerInfoError> {
        // Convert the path-like to a path
        let path = path.as_ref();

        // Open a file
        let handle = match File::create(path) {
            Ok(handle) => handle,
            Err(err)   => { return Err(LocalContainerInfoError::FileCreateError{ path: path.to_path_buf(), err }); }
        };

        // Use ::to_write() to deal with the actual writing
        self.to_writer(handle)
    }

    /// Writes the LocalContainerInfo to the given writer.
    /// 
    /// **Generic types**
    ///  * `W`: The type of the writer, which implements Write.
    /// 
    /// **Arguments**
    ///  * `writer`: The writer to write to. Will be consumed.
    /// 
    /// **Returns**  
    /// Nothing on success, or a LocalContainerInfoError otherwise.
    pub fn to_writer<W: Write>(&self, writer: W) -> Result<(), LocalContainerInfoError> {
        // Simply write with serde
        match serde_yaml::to_writer(writer, self) {
            Ok(())   => Ok(()),
            Err(err) => Err(LocalContainerInfoError::FileWriteError{ err }),
        }
    }
}

impl From<ContainerInfo> for LocalContainerInfo {
    fn from(container_info: ContainerInfo) -> Self {
        Self {
            name       : container_info.name,
            kind       : container_info.kind,
            entrypoint : container_info.entrypoint,
            actions    : container_info.actions,
            types      : container_info.types.unwrap_or_default(),
        }
    }
}

impl From<&ContainerInfo> for LocalContainerInfo {
    fn from(container_info: &ContainerInfo) -> Self {
        Self {
            name       : container_info.name.clone(),
            kind       : container_info.kind,
            entrypoint : container_info.entrypoint.clone(),
            actions    : container_info.actions.clone(),
            types      : container_info.types.as_ref().cloned().unwrap_or_default(),
        }
    }
}



/// Specifies the contents of a container info YAML file. Note that this is only the file the user creates.
#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    /// The name/programming ID of this package.
    pub name        : String,
    /// The version of this package.
    pub version     : Version,
    /// The kind of this package.
    pub kind        : PackageKind,
    /// The list of owners of this package.
    pub owners      : Option<Vec<String>>,
    /// A short description of the package.
    pub description : Option<String>,

    /// The functions that this package supports.
    pub actions    : Map<Action>,
    /// The entrypoint of the image
    pub entrypoint : Entrypoint,
    /// The types that this package adds.
    pub types      : Option<Map<Type>>,

    /// The base image to use for the package image.
    pub base         : Option<String>,
    /// The dependencies, as install commands for sudo apt-get install -y <...>
    pub dependencies : Option<Vec<String>>,
    /// Any environment variables that the user wants to be set
    pub environment  : Option<Map<String>>,
    /// The list of additional files to copy to the image
    pub files        : Option<Vec<String>>,
    /// An extra script to run to initialize the working directory
    pub initialize   : Option<Vec<String>>,
    /// An extra script to run to install the image(?)
    pub install      : Option<Vec<String>>,
}

#[allow(unused)]
impl ContainerInfo {
    /// **Edited: now returning ContainerInfoErrors.**
    /// 
    /// Returns a ContainerInfo by constructing it from the file at the given path.
    /// 
    /// **Generic types**
    ///  * `P`: The Path-like type of the given target location.
    /// 
    /// **Arguments**
    ///  * `path`: The path to the container info file.
    /// 
    /// **Returns**  
    /// The newly constructed ContainerInfo instance on success, or a ContainerInfoError upon failure.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<ContainerInfo, ContainerInfoError> {
        // Convert the path-like to a path
        let path = path.as_ref();

        // Read the contents in one go
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(err)     => { return Err(ContainerInfoError::FileReadError{ path: path.to_path_buf(), err }); }
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
            Err(err)   => Err(ContainerInfoError::ParseError{ err }),
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
            Err(err)   => Err(ContainerInfoError::ParseError{ err }),
        }
    }



    /// Writes the ContainerInfo to the given location.
    /// 
    /// **Generic types**
    ///  * `P`: The Path-like type of the given target location.
    /// 
    /// **Arguments**
    ///  * `path`: The target location to write to the LocalContainerInfo to.
    /// 
    /// **Returns**  
    /// Nothing on success, or a ContainerInfoError otherwise.
    pub fn to_path<P: AsRef<Path>>(&self, path: P) -> Result<(), ContainerInfoError> {
        // Convert the path-like to a path
        let path = path.as_ref();

        // Open a file
        let handle = match File::create(path) {
            Ok(handle) => handle,
            Err(err)   => { return Err(ContainerInfoError::FileCreateError{ path: path.to_path_buf(), err }); }
        };

        // Use ::to_write() to deal with the actual writing
        self.to_writer(handle)
    }

    /// Writes the ContainerInfo to the given writer.
    /// 
    /// **Generic types**
    ///  * `W`: The type of the writer, which implements Write.
    /// 
    /// **Arguments**
    ///  * `writer`: The writer to write to. Will be consumed.
    /// 
    /// **Returns**  
    /// Nothing on success, or a ContainerInfoError otherwise.
    pub fn to_writer<W: Write>(&self, writer: W) -> Result<(), ContainerInfoError> {
        // Simply write with serde
        match serde_yaml::to_writer(writer, self) {
            Ok(())   => Ok(()),
            Err(err) => Err(ContainerInfoError::FileWriteError{ err }),
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
