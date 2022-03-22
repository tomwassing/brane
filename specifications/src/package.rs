use crate::common::{Function, Type};
use crate::container::ContainerInfo;
use anyhow::Result;
use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value as JValue;
use serde_with::skip_serializing_none;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

type Map<T> = std::collections::HashMap<String, T>;


/* TIM */
/***** ERRORS *****/
/// Lists the errors that can occur for the PackageKind enum
#[derive(Debug)]
pub enum PackageKindError {
    /// We tried to convert a string to a PackageKind but failed
    IllegalKind{ skind: String },
}

impl PackageKindError {
    /// Static helper that collects a list of possible package kinds.
    /// 
    /// **Returns**  
    /// A string list of the possible package kinds to enter.
    fn get_package_kinds() -> String {
        let mut kinds = String::new();
        for kind in PackageKind::iter() {
            if !kinds.is_empty() { kinds += ", "; }
            kinds += &format!("'{}'", kind);
        }
        kinds
    }
}

impl std::fmt::Display for PackageKindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageKindError::IllegalKind{ skind } => write!(f, "'{}' is not a valid package type; possible types are {}", skind, Self::get_package_kinds()),
        }
    }
}

impl std::error::Error for PackageKindError {}



/// Lists the errors that can occur for the PackageInfo struct
#[derive(Debug)]
pub enum PackageInfoError {
    /// We could not parse a given yaml string as a PackageInfo
    IllegalString{ err: serde_yaml::Error },
    /// We could not parse a given yaml file as a PackageInfo
    IllegalFile{ path: PathBuf, err: serde_yaml::Error },
    /// We could not parse a given set of JSON-encoded PackageInfos
    IllegalJsonValue{ err: serde_json::Error },
    /// Could not open the file we wanted to load
    IOError{ path: PathBuf, err: std::io::Error },
}

impl std::fmt::Display for PackageInfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageInfoError::IllegalString{ err }     => write!(f, "Cannot construct PackageInfo object from YAML string: {}", err),
            PackageInfoError::IllegalFile{ path, err } => write!(f, "Cannot construct PackageInfo object from YAML file '{}': {}", path.display(), err),
            PackageInfoError::IllegalJsonValue{ err }  => write!(f, "Cannot construct PackageInfo object from JSON value: {}", err),
            PackageInfoError::IOError{ path, err }     => write!(f, "Error while trying to read PackageInfo file '{}': {}", path.display(), err),
        }
    }
}

impl std::error::Error for PackageInfoError {}



/// Lists the errors that can occur for the PackageIndex struct
#[derive(Debug)]
pub enum PackageIndexError{
    /// A package/version combination has already been loaded into the PackageIndex
    DuplicatePackage{ name: String, version: String },
    /// Could not parse a version string as one
    IllegalVersion{ package: String, raw: String, err: semver::Error },

    /// We could not do a request to some server to get a JSON file
    RequestFailed{ url: String, err: reqwest::Error },
    /// A HTTP request returned a non-200 status code
    ResponseNot200{ url: String, status: reqwest::StatusCode },
    /// Coult not parse a given remote JSON file as a PackageIndex
    IllegalJsonFile{ url: String, err: reqwest::Error },

    /// Could not parse a given reader with JSON data as a PackageIndex
    IllegalJsonReader{ err: serde_json::Error },
    /// Could not correct parse the JSON as a list of PackageInfo structs
    IllegalPackageInfos{ err: PackageInfoError },
    /// Could not open the file we wanted to load
    IOError{ path: PathBuf, err: std::io::Error },
}

impl std::fmt::Display for PackageIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageIndexError::DuplicatePackage{ name, version }   => write!(f, "Encountered duplicate version {} of package '{}'", version, name),
            PackageIndexError::IllegalVersion{ package, raw, err } => write!(f, "Could not parse version string '{}' in package.yml of package '{}' to a Version: {}", raw, package, err),

            PackageIndexError::RequestFailed{ url, err }     => write!(f, "Could not send a request to '{}': {}", url, err),
            PackageIndexError::ResponseNot200{ url, status } => write!(f, "Request sent to '{}' returned status {}", url, status),
            PackageIndexError::IllegalJsonFile{ url, err } => write!(f, "Cannot construct PackageIndex object from JSON file at '{}': {}", url, err),

            PackageIndexError::IllegalJsonReader{ err }    => write!(f, "Cannot construct PackageIndex object from JSON reader: {}", err),
            PackageIndexError::IllegalPackageInfos{ err }  => write!(f, "Cannot parse list of PackageInfos: {}", err),
            PackageIndexError::IOError{ path, err }        => write!(f, "Error while trying to read PackageIndex file '{}': {}", path.display(), err),
        }
    }
}

impl std::error::Error for PackageIndexError {}





/***** ENUMS *****/
/// Enum that lists possible package types
#[derive(Debug, Deserialize, Clone, Copy, EnumIter, PartialEq, Serialize)]
pub enum PackageKind {
    /// The package is an executable package (wrapping some other language or code)
    #[serde(rename = "ecu")]
    Ecu,
    /// The package is implemented using the Open API Standard
    #[serde(rename = "oas")]
    Oas,
    /// The package is an external DSL function
    #[serde(rename = "dsl")]
    Dsl,
    /// The package is an CWL job(?)
    #[serde(rename = "cwl")]
    Cwl,
}

impl PackageKind {
    /// Returns a more understandable name for the PackageKinds.
    pub fn pretty(&self) -> &str {
        match self {
            PackageKind::Ecu => "code package",
            PackageKind::Oas => "Open API Standard package",
            PackageKind::Dsl => "BraneScript/Bakery package",
            PackageKind::Cwl => "CWL package",
        }
    }
}

impl std::str::FromStr for PackageKind {
    type Err = PackageKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Convert to lowercase
        let ls = s.to_lowercase();

        // Match
        match ls.as_str() {
            "ecu" => Ok(PackageKind::Ecu),
            "oas" => Ok(PackageKind::Oas),
            "dsl" => Ok(PackageKind::Dsl),
            "cwl" => Ok(PackageKind::Cwl),
            _     => Err(PackageKindError::IllegalKind{ skind: ls }),
        }
    }
}

impl std::convert::From<PackageKind> for String {
    fn from(value: PackageKind) -> String {
        String::from(&value)
    }
}

impl std::convert::From<&PackageKind> for String {
    fn from(value: &PackageKind) -> String {
        match value {
            PackageKind::Ecu => String::from("ecu"),
            PackageKind::Oas => String::from("oas"),
            PackageKind::Dsl => String::from("dsl"),
            PackageKind::Cwl => String::from("cwl"),
        }
    }
}

impl std::fmt::Display for PackageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}
/*******/





#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
    pub created: DateTime<Utc>,
    pub description: String,
    pub detached: bool,
    pub functions: Option<Map<Function>>,
    pub id: Uuid,
    /* TIM */
    // pub kind: String,
    pub kind: PackageKind,
    /*******/
    pub name: String,
    pub owners: Vec<String>,
    pub types: Option<Map<Type>>,
    pub version: String,
}

#[allow(unused)]
impl PackageInfo {
    ///
    ///
    ///
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        version: String,
        description: String,
        detached: bool,
        /* TIM */
        // kind: String,
        kind: PackageKind,
        /*******/
        owners: Vec<String>,
        functions: Option<Map<Function>>,
        types: Option<Map<Type>>,
    ) -> PackageInfo {
        let id = Uuid::new_v4();
        let created = Utc::now();

        PackageInfo {
            created,
            description,
            detached,
            functions,
            id,
            kind,
            name,
            owners,
            types,
            version,
        }
    }

    /* TIM */
    /// **Edited: changed to return appropriate errors. Also added docstring.**
    /// 
    /// Tries to create a new PackageInfo from the given (yaml-formatted) file.
    /// 
    /// **Arguments**
    ///  * `path`: The path to load.
    /// 
    /// **Returns**  
    /// The new PackageInfo upon success, or a PackageInfoError detailling why if it failed.
    pub fn from_path(path: PathBuf) -> Result<PackageInfo, PackageInfoError> {
        // Read the file first
        let contents = match fs::read_to_string(&path) {
            Ok(values)  => values,
            Err(reason) => { return Err(PackageInfoError::IOError{ path, err: reason }); }
        };

        // Next, delegate actual reading to from_string
        match PackageInfo::from_string(contents) {
            Ok(result)                                  => Ok(result),
            Err(PackageInfoError::IllegalString{ err }) => Err(PackageInfoError::IllegalFile{ path, err }),
            Err(reason)                                 => Err(reason),
        }
    }
    /*******/

    /* TIM */
    /// **Edited: changed to return appropriate errors. Also added docstring.**
    /// 
    /// Tries to create a new PackageInfo from the given (yaml-formatted) string.
    /// 
    /// **Arguments**
    ///  * `contents`: The string that contains the contents for the PackageInfo.
    /// 
    /// **Returns**  
    /// The new PackageInfo upon success, or a PackageInfoError detailling why if it failed.
    pub fn from_string(contents: String) -> Result<PackageInfo, PackageInfoError> {
        // Try to parse using serde
        match serde_yaml::from_str(&contents) {
            Ok(result)  => Ok(result),
            Err(reason) => Err(PackageInfoError::IllegalString{ err: reason }),
        }
    }
    /*******/
}

impl From<ContainerInfo> for PackageInfo {
    #[inline]
    fn from(container: ContainerInfo) -> Self { PackageInfo::from(&container) }
}

impl From<&ContainerInfo> for PackageInfo {
    fn from(container: &ContainerInfo) -> Self {
        // Construct function descriptions
        let mut functions = Map::<Function>::new();
        for (action_name, action) in &container.actions {
            let function_output = action.output.clone().unwrap_or_default();

            let arguments = action.input.clone().unwrap_or_default();
            let pattern = action.pattern.clone();
            let return_type = match function_output.first() {
                Some(output) => output.data_type.to_string(),
                None         => String::from("unit"),
            };

            let function = Function::new(arguments, pattern, return_type);
            functions.insert(action_name.clone(), function);
        }

        // Create and write a package.yml file.
        PackageInfo::new(
            container.name.clone(),
            container.version.clone(),
            container.description.clone().unwrap_or_default(),
            container.entrypoint.kind == *"service",
            PackageKind::Ecu,
            vec![],
            Some(functions),
            container.types.clone(),
        )
    }
}



#[derive(Debug, Clone, Default)]
pub struct PackageIndex {
    pub packages: Map<PackageInfo>,
    pub standard: Map<PackageInfo>,
    pub versions: Map<Vec<Version>>,
}

impl PackageIndex {
    ///
    ///
    ///
    pub fn empty() -> Self {
        let packages = Map::<PackageInfo>::new();
        let versions = Map::<Vec<Version>>::new();

        PackageIndex::new(packages, versions)
    }

    ///
    ///
    ///
    pub fn new(
        packages: Map<PackageInfo>,
        mut versions: Map<Vec<Version>>,
    ) -> Self {
        // Make sure the latest version can be retrieved with .first()
        for (_, p_versions) in versions.iter_mut() {
            p_versions.sort();
            p_versions.reverse();
        }

        let standard = Map::default();
        PackageIndex {
            packages,
            standard,
            versions,
        }
    }

    /* TIM */
    /// **Edited: Returns PackageIndexErrors now.**
    ///
    /// Tries to construct a new PackageIndex from the application file at the given path.
    /// 
    /// **Arguments**
    ///  * `path`: Path to the application file.
    /// 
    /// **Returns**  
    /// The new PackageIndex if it all went fine, or a PackageIndexError if it didn't.
    pub fn from_path(path: &Path) -> Result<Self, PackageIndexError> {
        // Try to open the referenced file
        let file = match File::open(path) {
            Ok(handle)  => handle,
            Err(reason) => { return Err(PackageIndexError::IOError{ path: PathBuf::from(path), err: reason }); }
        };

        // Wrap it in a bufreader and go to from_reader
        let buf_reader = BufReader::new(file);
        PackageIndex::from_reader(buf_reader)
    }
    /*******/

    /* TIM */
    /// **Edited: Returns PackageIndexErrors now.**
    ///
    /// Tries to construct a new PackageIndex from the given reader.
    /// 
    /// **Arguments**
    ///  * `r`: The reader that contains the data to construct the PackageIndex from.
    /// 
    /// **Returns**  
    /// The new PackageIndex if it all went fine, or a PackageIndexError if it didn't.
    pub fn from_reader<R: Read>(r: R) -> Result<Self, PackageIndexError> {
        // Try to parse using serde
        let v = match serde_json::from_reader(r) {
            Ok(value)   => value,
            Err(reason) => { return Err(PackageIndexError::IllegalJsonReader{ err: reason }); }
        };

        // Delegate the parsed JSON struct to the from_value one
        PackageIndex::from_value(v)
    }
    /*******/

    /* TIM */
    /// **Edited: Returns PackageIndexErrors now.**
    ///
    /// Tries to construct a new PackageIndex from a JSON file at the given URL.
    /// 
    /// **Arguments**
    ///  * `url`: The location of the JSON file to parse.
    /// 
    /// **Returns**  
    /// The new PackageIndex if it all went fine, or a PackageIndexError if it didn't.
    pub async fn from_url(url: &str) -> Result<Self, PackageIndexError> {
        // try to get the file
        let json = match reqwest::get(url).await {
            Ok(response) => if response.status() == reqwest::StatusCode::OK {
                // We have the request; now try to get it as json
                match response.json().await {
                    Ok(value)   => value,
                    Err(reason) => { return Err(PackageIndexError::IllegalJsonFile{ url: url.to_string(), err: reason }); }
                }
            } else {
                return Err(PackageIndexError::ResponseNot200{ url: url.to_string(), status: response.status() });
            },
            Err(reason) => { return Err(PackageIndexError::RequestFailed{ url: url.to_string(), err: reason }); },
        };

        // Done; pass the rest to the from_value() function
        PackageIndex::from_value(json)
    }
    /*******/

    /* TIM */
    /// **Edited: Returns PackageIndexErrors now.**
    ///
    /// Tries to construct a new PackageIndex from the given JSON-parsed value.
    /// 
    /// **Arguments**
    ///  * `v`: The JSON root value of the tree to parse.
    /// 
    /// **Returns**  
    /// The new PackageIndex if it all went fine, or a PackageIndexError if it didn't.
    pub fn from_value(v: JValue) -> Result<Self, PackageIndexError> {
        // Parse the known packages from the list of json values
        let known_packages: Vec<PackageInfo> = match serde_json::from_value(v) {
            Ok(pkgs)    => pkgs,
            Err(reason) => { return Err(PackageIndexError::IllegalPackageInfos{
                err: PackageInfoError::IllegalJsonValue{ err: reason },
            });}
        };

        // Construct the package index from the list of packages
        PackageIndex::from_packages(known_packages)
    }
    /*******/

    /* TIM */
    /// **Edited: Returns PackageIndexErrors now.**
    ///
    /// Tries to construct a new PackageIndex from a list of PackageInfos.
    /// 
    /// **Arguments**
    ///  * `known_packages`: List of PackageInfos to incorporate in the PackageIndex.
    /// 
    /// **Returns**  
    /// The new PackageIndex if it all went fine, or a PackageIndexError if it didn't.
    pub fn from_packages(known_packages: Vec<PackageInfo>) -> Result<Self, PackageIndexError> {
        // Construct the list of packages and of versions
        let mut packages = Map::<PackageInfo>::new();
        let mut versions = Map::<Vec<Version>>::new();
        for package in known_packages {
            // Compute the key for this package
            let key = format!("{}-{}", package.name, package.version);
            if packages.contains_key(&key) { return Err(PackageIndexError::DuplicatePackage{ name: package.name.clone(), version: package.version.clone() }); }
            packages.insert(key, package.clone());

            // Try to parse the package's version
            let version = match Version::parse(&package.version) {
                Ok(ver)     => ver,
                Err(reason) => { return Err(PackageIndexError::IllegalVersion{ package: package.name.clone(), raw: package.version.clone(), err: reason }); }
            };

            // With that done, insert it into the map of versions
            if let Some(p_versions) = versions.get_mut(&package.name) {
                p_versions.push(version);
            } else {
                versions.insert(package.name, vec![version]);
            }
        }

        // We have collected the list so we're done!
        Ok(PackageIndex::new(packages, versions))
    }
    /*******/

    ///
    ///
    ///
    pub fn get(
        &self,
        name: &str,
        version: Option<&Version>,
    ) -> Option<&PackageInfo> {
        let standard_package = self.standard.get(name);
        if standard_package.is_some() {
            return standard_package;
        }

        let version = if version.is_none() {
            self.get_latest_version(name)?
        } else {
            version?
        };

        self.packages.get(&format!("{}-{}", name, version))
    }

    ///
    ///
    ///
    fn get_latest_version(
        &self,
        name: &str,
    ) -> Option<&Version> {
        self.versions.get(name).map(|vs| vs.first()).unwrap_or(None)
    }
}
