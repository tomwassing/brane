/* ERRORS.rs
 *   by Lut99
 *
 * Created:
 *   11 Feb 2022, 13:09:23
 * Last edited:
 *   25 Mar 2022, 16:33:27
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Collects errors for the brane-let applications.
**/

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::path::PathBuf;

use crate::callback::CallbackError;
use specifications::container::LocalContainerInfoError;
use specifications::package::PackageKind;


/***** ERRORS *****/
/// Generic, top-level errors for the brane-let application.
#[derive(Debug)]
pub enum LetError {
    /// Could not launch the JuiceFS executable
    JuiceFSLaunchError{ command: String, err: std::io::Error },
    /// The JuiceFS executable didn't complete successfully
    JuiceFSError{ command: String, code: i32, stdout: String, stderr: String },

    /// Could not start the proxy redirector in the background
    RedirectorError{ address: String, err: String },
    /// Failed to connect to a remote callback while asked
    CallbackConnectError{ address: String, err: CallbackError },

    /// Could not decode input arguments with Base64
    ArgumentsBase64Error{ err: base64::DecodeError },
    /// Could not decode input arguments as UTF-8
    ArgumentsUTF8Error{ err: std::string::FromUtf8Error },
    /// Could not decode input arguments with JSON
    ArgumentsJSONError{ err: serde_json::Error },

    /// Could not load a ContainerInfo file.
    LocalContainerInfoError{ path: PathBuf, err: LocalContainerInfoError },
    /// Could not load a PackageInfo file.
    PackageInfoError{ err: anyhow::Error },
    /// Missing the 'functions' property in the package info YAML
    MissingFunctionsProperty{ path: PathBuf },
    /// The requested function is not part of the package that this brane-let is responsible for
    UnknownFunction{ function: String, package: String, kind: PackageKind },
    /// We're missing a required parameter in the function
    MissingInputArgument{ function: String, package: String, kind: PackageKind, name: String },
    /// An argument has an incompatible type
    IncompatibleTypes{ function: String, package: String, kind: PackageKind, name: String, expected: String, got: String },
    /// Could not start the init.sh workdirectory preparation script
    WorkdirInitLaunchError{ command: String, err: std::io::Error },
    /// The init.sh workdirectory preparation script returned a non-zero exit code
    WorkdirInitError{ command: String, code: i32, stdout: String, stderr: String },

    /// Could not canonicalize the entrypoint file's path
    EntrypointPathError{ path: PathBuf, err: std::io::Error },
    /// We encountered two arguments with indistinguishable names
    DuplicateArgument{ name: String },
    /// We encountered an array element with indistringuishable name from another environment variable
    DuplicateArrayArgument{ array: String, elem: usize, name: String },
    /// We encountered a struct field with indistringuishable name from another environment variable
    DuplicateStructArgument{ sname: String, field: String, name: String },
    /// The user tried to pass an unsupported type to a function
    UnsupportedType{ argument: String, elem_type: String },
    /// The user tried to give us a nested array, but that's unsupported for now.
    UnsupportedNestedArray{ elem: usize },
    /// The user tried to give us an array with (for now) unsupported element types.
    UnsupportedArrayElement{ elem: usize, elem_type: String },
    /// The user tried to give us a struct with a nested array.
    UnsupportedStructArray{ name: String, field: String, },
    /// The user tried to pass a nested Directory or File argument without 'url' property.
    UnsupportedNestedStruct{ name: String, field: String, },
    /// The user tried to pass a Struct with a general unsupported type.
    UnsupportedStructField{ name: String, field: String, elem_type: String },
    /// The user tried to pass a nested Directory or File argument without 'url' property.
    IllegalNestedURL{ name: String, field: String, },
    /// We got an error launching the package
    PackageLaunchError{ command: String, err: std::io::Error },

    /// The given Open API Standard file does not parse as OAS
    IllegalOasDocument{ path: PathBuf, err: anyhow::Error },

    /// Somehow, we got an error while waiting for the subprocess
    PackageRunError{ err: std::io::Error },
    /// The subprocess' stdout wasn't opened successfully
    ClosedStdout,
    /// The subprocess' stderr wasn't opened successfully
    ClosedStderr,
    /// Could not open stdout
    StdoutReadError{ err: std::io::Error },
    /// Could not open stderr
    StderrReadError{ err: std::io::Error },

    /// Something went wrong while decoding the package output
    DecodeError{ stdout: String, err: DecodeError },
    /// Encountered more than one output from the function
    UnsupportedMultipleOutputs{ n: usize },

    /// Could not write the resulting value to JSON
    ResultJSONError{ value: String, err: serde_json::Error },
}

impl Display for LetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            LetError::JuiceFSLaunchError{ command, err }            => write!(f, "Could not run JuiceFS command '{}': {}", command, err),
            LetError::JuiceFSError{ command, code, stdout, stderr } => write!(f, "JuiceFS command '{}' returned exit code {}:\n\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n\n", command, code, (0..80).map(|_| '-').collect::<String>(), stdout, (0..80).map(|_| '-').collect::<String>(), (0..80).map(|_| '-').collect::<String>(), stderr,(0..80).map(|_| '-').collect::<String>()),

            LetError::RedirectorError{ address, err }      => write!(f, "Could not start redirector to '{}' in the background: {}", address, err),
            LetError::CallbackConnectError{ address, err } => write!(f, "Could not connect to remote callback node at '{}': {}", address, err),

            LetError::ArgumentsBase64Error{ err } => write!(f, "Could not decode input arguments as Base64: {}", err),
            LetError::ArgumentsUTF8Error{ err }   => write!(f, "Could not decode input arguments as UTF-8: {}", err),
            LetError::ArgumentsJSONError{ err }   => write!(f, "Could not parse input arguments as JSON: {}", err),

            LetError::LocalContainerInfoError{ path, err }                              => write!(f, "Could not load local container information file '{}': {}", path.display(), err),
            LetError::PackageInfoError{ err }                                           => write!(f, "Could not parse package information file from Open-API document: {}", err),
            LetError::MissingFunctionsProperty{ path }                                  => write!(f, "Missing property 'functions' in package information file '{}'", path.display()),
            LetError::UnknownFunction{ function, package, kind }                        => write!(f, "Unknown function '{}' in package '{}' ({})", function, package, kind.pretty()),
            LetError::MissingInputArgument{ function, package, kind, name }             => write!(f, "Parameter '{}' not specified for function '{}' in package '{}' ({})", name, function, package, kind.pretty()),
            LetError::IncompatibleTypes{ function, package, kind, name, expected, got } => write!(f, "Type check failed for parameter '{}' of function '{}' in package '{}' ({}): expected {}, got {}", name, function, package, kind.pretty(), expected, got),
            LetError::WorkdirInitLaunchError{ command, err }                            => write!(f, "Could not run init.sh ('{}'): {}", command, err),
            LetError::WorkdirInitError{ command, code, stdout, stderr }                 => write!(f, "init.sh ('{}') returned exit code {}:\n\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n\n", command, code, (0..80).map(|_| '-').collect::<String>(), stdout, (0..80).map(|_| '-').collect::<String>(), (0..80).map(|_| '-').collect::<String>(), stderr,(0..80).map(|_| '-').collect::<String>()),

            LetError::EntrypointPathError{ path, err }                 => write!(f, "Could not canonicalize path '{}': {}", path.display(), err),
            LetError::DuplicateArgument{ name }                        => write!(f, "Encountered duplicate function argument '{}'; make sure your names don't conflict in case-insensitive scenarios either", name),
            LetError::DuplicateArrayArgument{ array, elem, name }      => write!(f, "Element {} of array '{}' has the same name as environment variable '{}'; remember that arrays generate new arguments for each element", elem, array, name),
            LetError::DuplicateStructArgument{ sname, field, name }    => write!(f, "Field '{}' of struct '{}' has the same name as environment variable '{}'; remember that structs generate new arguments for each field", field, sname, name),
            LetError::UnsupportedType{ argument, elem_type }           => write!(f, "Argument '{}' has type '{}'; this type is not (yet) supported, please use other types", argument, elem_type),
            LetError::UnsupportedNestedArray{ elem }                   => write!(f, "Element {} of array is an array; nested arrays are not (yet) supported, please use flat arrays only", elem),
            LetError::UnsupportedArrayElement{ elem, elem_type }       => write!(f, "Element {} of array has type '{}'; this type is not (yet) supported in arrays, please use other types", elem, elem_type),
            LetError::UnsupportedStructArray{ name, field }            => write!(f, "Field '{}' of struct '{}' is an array; nested arrays in structs are not (yet) supported, please pass arrays separately as flat arrays", field, name),
            LetError::UnsupportedNestedStruct{ name, field }           => write!(f, "Field '{}' of struct '{}' is a non-File, non-Directory struct; nested structs are not (yet) supported, please pass structs separately", field, name),
            LetError::UnsupportedStructField{ name, field, elem_type } => write!(f, "Field '{}' of struct '{}' has type '{}'; this type is not (yet) supported in structs, please use other types", field, name, elem_type),
            LetError::IllegalNestedURL{ name, field }                  => write!(f, "Field '{}' of struct '{}' is a Directory or a File struct, but misses the 'URL' field", field, name),
            LetError::PackageLaunchError{ command, err }               => write!(f, "Could not run nested package call '{}': {}", command, err),

            LetError::IllegalOasDocument{ path, err } => write!(f, "Could not parse OpenAPI specification '{}': {}", path.display(), err),

            LetError::ClosedStdout           => write!(f, "Could not open subprocess stdout"),
            LetError::ClosedStderr           => write!(f, "Could not open subprocess stdout"),
            LetError::StdoutReadError{ err } => write!(f, "Could not read from stdout: {}", err),
            LetError::StderrReadError{ err } => write!(f, "Could not read from stderr: {}", err),
            LetError::PackageRunError{ err } => write!(f, "Could not get package run status: {}", err),

            LetError::DecodeError{ stdout, err }      => write!(f, "Could not parse package stdout: {}\n\nstdout:\n{}\n{}\n{}\n\n", err, (0..80).map(|_| '-').collect::<String>(), stdout, (0..80).map(|_| '-').collect::<String>()),
            LetError::UnsupportedMultipleOutputs{ n } => write!(f, "Function return {} outputs; this is not (yet) supported, please return only one", n),

            LetError::ResultJSONError{ value, err } => write!(f, "Could not serialize value '{}' to JSON: {}", value, err),
        }
    }
}

impl Error for LetError {}



/// Defines errors that can occur during decoding.
#[derive(Debug)]
pub enum DecodeError {
    /// The input was not valid YAML
    InvalidYAML{ err: yaml_rust::ScanError },
    /// The input was not valid JSON
    InvalidJSON{ err: serde_json::Error },

    /// The input is not a valid Hash, i.e., not a valid object (I think)
    NotAHash,
    /// Some returned output argument was missing from what the function reported
    MissingOutputArgument{ name: String },
    /// Some returned output argument has an incorrect type
    OutputTypeMismatch{ name: String, expected: String, got: String },
    /// A given output has a given class type defined, but we don't know about it
    UnknownClassType{ name: String, class_name: String },

    /// Some output struct did not have all its properties defined.
    MissingStructProperty{ name: String, class_name: String, property_name: String },
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            DecodeError::InvalidYAML{ err } => write!(f, "Invalid YAML: {}", err),
            DecodeError::InvalidJSON{ err } => write!(f, "Invalid JSON: {}", err),

            DecodeError::NotAHash                                  => write!(f, "Top-level YAML is not a valid hash"),
            DecodeError::MissingOutputArgument{ name }             => write!(f, "Missing output argument '{}' in function output", name),
            DecodeError::OutputTypeMismatch{ name, expected, got } => write!(f, "Function output '{}' has type '{}', but expected type '{}'", name, got, expected),
            DecodeError::UnknownClassType{ name, class_name }      => write!(f, "Function output '{}' has object type '{}', but that object type is undefined", name, class_name),

            DecodeError::MissingStructProperty{ name, class_name, property_name } => write!(f, "Function output '{}' has object type '{}', but is missing property '{}'", name, class_name, property_name),
        }
    }
}

impl Error for DecodeError {}
