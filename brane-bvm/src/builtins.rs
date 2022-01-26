use crate::objects::{Class, Object};
use crate::{
    executor::{ServiceState, VmExecutor, ExecutorError},
    stack::Slot,
};
use broom::Heap;
use fnv::FnvHashMap;
use specifications::common::Value;

/* TIM */
// const BUILTIN_PRINT_NAME: &str = "print";
// const BUILTIN_PRINT_CODE: u8 = 0x01;

// const BUILTIN_WAIT_UNTIL_STARTED_CODE: u8 = 0x02;
// const BUILTIN_WAIT_UNTIL_DONE_CODE: u8 = 0x03;

// const BUILTIN_SERVICE_NAME: &str = "Service";

/// Defines the builtin function codes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuiltinFunction {
    /// Meta enum value for when no builtin is defined
    Undefined,

    /// Defines a print operation to stdout
    Print = 0x01,
    /// Waits until a job has been started
    WaitUntilStarted = 0x02,
    /// Waits until a job has been done
    WaitUntilDone = 0x03,
}

impl BuiltinFunction {
    /// Returns the string representation of this Builtin
    /// 
    /// **Returns**  
    /// The string that represents the given Builtin, or else None if the string isn't meant to be accessed directly.
    pub fn signature(&self) -> Option<&str> {
        match self {
            BuiltinFunction::Print => Some("print"),
            _                      => None,
        }
    }
}

impl From<u8> for BuiltinFunction {
    fn from(value: u8) -> Self {
        match value {
            0x01 => BuiltinFunction::Print,
            0x02 => BuiltinFunction::WaitUntilStarted,
            0x03 => BuiltinFunction::WaitUntilDone,
            _    => BuiltinFunction::Undefined,
        }
    }
}

impl std::fmt::Display for BuiltinFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            BuiltinFunction::Undefined        => write!(f, "<undefined>"),
            BuiltinFunction::Print            => write!(f, "print[{}]", *self as u8),
            BuiltinFunction::WaitUntilStarted => write!(f, "wait_until_started[{}]", *self as u8),
            BuiltinFunction::WaitUntilDone    => write!(f, "wait_until_done[{}]", *self as u8),
        }
    }
}



/// Defines the builtin classes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuiltinClass {
    /// The Service class, which represents an asynchronous function
    Service,
}

impl std::fmt::Display for BuiltinClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinClass::Service   => write!(f, "Service"),
        }
    }
}



/// Enum for errors relating to the buildins.
#[derive(Debug)]
pub enum BuiltinError {
    /// Error for when remote printing failed
    ClientTxError{ text: String, err: ExecutorError },

    /// Error for when an opcode is unknown
    UnknownOpcode{ opcode: u8 },
    /// Error for when we're missing a field in the instance struct
    InvalidInstanceError{ builtin: BuiltinFunction },
    /// Error for when an external function could not be scheduled
    ScheduleError{ builtin: BuiltinFunction, function: String, err: ExecutorError },

    /// Error for when there are too few arguments passed to a builtin
    NotEnoughArgumentsError{ builtin: BuiltinFunction, expected: usize, got: usize },
    /// Error for when a builtin got too much arguments
    TooManyArgumentsError{ builtin: BuiltinFunction, expected: usize, got: usize },
}

impl std::fmt::Display for BuiltinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinError::ClientTxError{ text, err } => write!(f, "print: Could not write '{}' to stdout: {}", text, err),

            BuiltinError::UnknownOpcode{ opcode } => write!(f, "Unknown builtin opcode '{}'", opcode),
            BuiltinError::InvalidInstanceError{ builtin } => write!(f, "{}: Argument is not an Instance description (either not a struct or doesn't have the 'identifier' field)", builtin),
            BuiltinError::ScheduleError{ builtin, function, err } => write!(f, "{}: Could not schedule function '{}' for execution: {}", builtin, function, err),

            BuiltinError::NotEnoughArgumentsError{ builtin, expected, got } => write!(f, "{}: Not enough arguments (got {}, expected {})", builtin, got, expected),
            BuiltinError::TooManyArgumentsError{ builtin, expected, got } => write!(f, "{}: Too many arguments (got {}, expected {})", builtin, got, expected),
        }
    }
}

impl std::error::Error for BuiltinError {}
/*******/


///
///
///
pub fn register(
    globals: &mut FnvHashMap<String, Slot>,
    heap: &mut Heap<Object>,
) {
    // Classes
    let service_name = format!("{}", BuiltinClass::Service);
    let service = heap.insert(class(service_name.clone())).into_handle();
    globals.insert(service_name, Slot::Object(service));

    // Functions
    globals.insert(BuiltinFunction::Print.signature().unwrap().to_string(), Slot::BuiltIn(BuiltinFunction::Print));
}

///
///
///
fn class(name: String) -> Object {
    Object::Class(Class {
        name,
        methods: Default::default(),
    })
}

/* TIM */
/// **Edited: change doccomment + now returning errors for all builtins**
///
/// Calls a builtin function.
/// 
/// **Arguments**
///  * `builtin`: The opcode for the builtin to call.
///  * `arguments`: The arguments for this builtin, as a list of Values
///  * `executor`: The executor to run external functions on and to communicate with the client with
///  * `_location`: The location where the external buildin will be run at (only here for compatibility reasons)
/// 
/// **Returns**  
/// The return Value of the builtin on success, or a BuiltinError if it failed.
pub async fn call<E>(
    builtin: BuiltinFunction,
    arguments: Vec<Value>,
    executor: &E,
    _location: Option<String>,
) -> Result<Value, BuiltinError>
where
    E: VmExecutor,
{
    match builtin {
        BuiltinFunction::Print => {
            // Check if the number of arguments is correct
            if arguments.len() < 1 { return Err(BuiltinError::NotEnoughArgumentsError{ builtin: BuiltinFunction::Print, expected: 1, got: 0 }); }
            else if arguments.len() > 1 { return Err(BuiltinError::TooManyArgumentsError{ builtin: BuiltinFunction::Print, expected: 1, got: arguments.len() }); }

            // Get the argument for this builtin
            let value = arguments.first().unwrap();
            // Get the string representation of the value
            let text = value.to_string();

            // Delegate printing to executor.
            if let Err(reason) = executor.stdout(text.clone()).await { return Err(BuiltinError::ClientTxError{ text: text, err: reason }); }

            // Success!
            Ok(Value::Unit)
        }
        BuiltinFunction::WaitUntilStarted => {
            wait_until_state(BuiltinFunction::WaitUntilStarted, &arguments, executor, ServiceState::Started).await
        }
        BuiltinFunction::WaitUntilDone => {
            wait_until_state(BuiltinFunction::WaitUntilDone, &arguments, executor, ServiceState::Done).await
        }
        _ => { return Err(BuiltinError::UnknownOpcode{ opcode: 0 }); },
    }
}
/*******/

/* TIM */
/// Helper function that starts a shared job and waits until the desired status has been reached.  
/// The job is read from the list of arguments this function got passed to it.
/// 
/// **Arguments**
///  * `builtin`: The name of the builtin that calls this function.
///  * `arguments`: The list of arguments passed to the builtin calling this function.
///  * `executor`: The Executor to schedule the job on.
///  * `desired_state`: The desired state to block until.
/// 
/// **Returns**  
/// Value::Unit on success or a BuiltinError describing what happened otherwise
async fn wait_until_state<E>(builtin: BuiltinFunction, arguments: &Vec<Value>, executor: &E, desired_state: ServiceState) -> Result<Value, BuiltinError> 
    where E: VmExecutor
{
    // Check if the number of arguments is correct
    if arguments.len() < 1 { return Err(BuiltinError::NotEnoughArgumentsError{ builtin: builtin, expected: 1, got: 0 }); }
    else if arguments.len() > 1 { return Err(BuiltinError::TooManyArgumentsError{ builtin: builtin, expected: 1, got: arguments.len() }); }

    // Get its only argument as a Struct
    let instance = arguments.first().unwrap();
    if let Value::Struct { properties, .. } = instance {
        // Parse the identifier of the instance
        let identifier = properties.get("identifier");
        if let None = identifier { return Err(BuiltinError::InvalidInstanceError{ builtin: builtin }); }
        let identifier = identifier.unwrap().to_string();

        // Schedule the job for execution
        // TODO: Doesn't seem to be implemented in all executors, so need to know more about the errors
        if let Err(reason) = executor.wait_until(identifier.clone(), desired_state).await {
            return Err(BuiltinError::ScheduleError{ builtin: builtin, function: identifier, err: reason });
        }
    } else {
        return Err(BuiltinError::InvalidInstanceError{ builtin: builtin });
    }

    // Done
    Ok(Value::Unit)
}
/*******/
