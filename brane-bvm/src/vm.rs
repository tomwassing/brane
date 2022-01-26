use std::cmp::max;

use crate::frames::{CallFrame, CallFrameError};
use crate::objects::Class;
use crate::stack::{Slot, Stack, StackError};
use crate::{
    builtins,
    builtins::BuiltinFunction,
    builtins::BuiltinError,
    bytecode::{opcodes::*, FunctionMut},
    executor::{VmExecutor, ExecutorError},
    objects::Object,
    objects::{Array, Instance},
    objects::ObjectError,
};
use broom::{Handle, Heap};
use fnv::FnvHashMap;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smallvec::SmallVec;
use specifications::common::{FunctionExt, Value, Typed};
use specifications::package::PackageIndex;
use tokio::runtime::Runtime;


/* TIM */
/// Public enum containing VM execution errors
#[derive(Debug)]
pub enum VmError {
    // /// Meta enum used for testing error passing
    // Test,
    /// Error that notifies the user they cannot use parallel yet
    ParallelNotImplementedError,

    /// Error for when try to flip the sign of a non-numeric value
    NotNegatable{ target: String },
    /// Error for when we try to compare two non-numeric values with each other (for math-like comparisons)
    NotComparable{ lhs: String, rhs: String },
    /// Error for when the two most recent values on the stack are not addable together (either numerically or as strings)
    NotAddable{ lhs: String, rhs: String },
    /// Error for when the two most recent values on the stack are not subtractable
    NotSubtractable{ lhs: String, rhs: String },
    /// Error for when the two most recent values on the stack are not multiplicable
    NotMultiplicable{ lhs: String, rhs: String },
    /// Error for when the two most recent values on the stack are not divisible
    NotDivisible{ lhs: String, rhs: String },
    /// Error for when the user tries to index a non-Array object
    IllegalIndexError{ target: String },
    /// Error for when the user uses a dot ('.') on a non-object
    IllegalDotError{ target: String },
    /// A bit more specific error for when the user uses a method on a non-object
    MethodDotError{ target: String },
    /// Error for when the user uses an illegal property type for an instance
    IllegalPropertyError{ target: String },
    /// Error for when we try to import an illegal type of value
    IllegalImportError{ target: String },
    /// Error for when we use the new operation on a non-class type
    IllegalNewError{ target: String },
    /// Error for when we encounter a non-function type as a parallel branch
    IllegalBranchError{ target: String },
    /// Error for when we call return() outside of a function and it doesn't stop the global context
    IllegalReturnError,

    /// Error for when the given opcode is unknown
    UndefinedOpcodeError{ opcode: u8 },
    /// Error for when an import refers an unknown package
    UndefinedImportError{ package: String },
    /// Error for when a global has an incorrect identifier
    IllegalGlobalIdentifierError{ target: String },
    /// Error for when a global is unknown to us
    UndefinedGlobalError{ identifier: String },
    /// Error for when an instance does not have the given property
    UndefinedPropertyError{ instance: String, property: String },
    /// Error for when the method does not belong to the instance
    UndefinedMethodError{ class: String, method: String },
    /// Error for when we encounter a Service, but is has a non-service related method
    IllegalServiceMethod{ method: String },

    /// Error for when a given function does not have enough arguments on the stack before calling
    FunctionArityError{ name: String, got: u8, expected: u8 },
    /// Error for when a given array does not have enough values on the stack
    ArrayArityError{ got: u8, expected: u8 },
    /// Error for when a class is created but not enough properties are found on the stack
    ClassArityError{ name: String, got: u8, expected: u8 },
    /// Error for when a parellel operator does not have enough branches on the stack
    ParallelArityError{ got: u8, expected: u8 },

    /// Error for when a package has an unknown type
    UnsupportedPackageKindError{ name: String, kind: String },
    /// Error for when an Array index goes out of bounds
    ArrayOutOfBoundsError{ index: usize, max: usize },

    /// Error for when we want to resolve some object to the heap but it doesn't exist
    DanglingHandleError,

    /// Could not read an opcode from the callframe
    CallFrameInstrError{ err: CallFrameError },
    /// Could not read an embedded 8-bit number from the callframe
    CallFrame8bitError{ what: String, err: CallFrameError },
    /// Could not read an embedded 16-bit number from the callframe
    CallFrame16bitError{ what: String, err: CallFrameError },
    /// Could not read a constant from the callframe
    CallFrameConstError{ what: String, err: CallFrameError },
    /// Could not read a value from the stack
    StackReadError{ what: String, err: StackError },
    /// An error occurred while working with objects
    ObjectError{ err: ObjectError },
    /// An error occurred while performing a builtin call
    BuiltinCallError{ builtin: BuiltinFunction, err: BuiltinError },
    /// An error occurred while performing an external call
    ExternalCallError{ function: String, err: ExecutorError },
    /// Could not send a message to the client
    ClientTxError{ err: ExecutorError },
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // VmError::Test                        => write!(f, "A test error occurred; if you can see this, then yay :D"),
            VmError::ParallelNotImplementedError => write!(f, "OP_PARALLEL has been deemed unsafe and will be reimplemented later."),

            VmError::NotNegatable{ target }         => write!(f, "Cannot negative value of type {}: expected a numeric value", target),
            VmError::NotComparable{ lhs, rhs }      => write!(f, "Cannot compare value of type {} with a value of type {}: expected two numeric values", lhs, rhs),
            VmError::NotAddable{ lhs, rhs }         => write!(f, "Cannot add value of type {} to a value of type {}: expected two numeric values or two strings", lhs, rhs),
            VmError::NotSubtractable{ lhs, rhs }    => write!(f, "Cannot subtract value of type {} with a value of type {}: expected two numeric values", lhs, rhs),
            VmError::NotMultiplicable{ lhs, rhs }   => write!(f, "Cannot multiply value of type {} with a value of type {}: expected two numeric values", lhs, rhs),
            VmError::NotDivisible{ lhs, rhs }       => write!(f, "Cannot divide value of type {} by a value of type {}: expected two numeric values", lhs, rhs),
            VmError::IllegalIndexError{ target }    => write!(f, "Cannot index type {}: expected an Array", target),
            VmError::IllegalDotError{ target }      => write!(f, "Cannot apply dot operator to type {}: expected an Instance", target),
            VmError::MethodDotError{ target }       => write!(f, "Cannot call a method on a {}: expected an Instance", target),
            VmError::IllegalPropertyError{ target } => write!(f, "Illegal object property {}: expected a string identifier", target),
            VmError::IllegalImportError{ target }   => write!(f, "Cannot import package of type {}: expected a string identifier", target),
            VmError::IllegalNewError{ target }      => write!(f, "Cannot instantiate object of type {}: expected a Class", target),
            VmError::IllegalBranchError{ target }   => write!(f, "Cannot run branch of type {} in parallel: expected a Function", target),
            VmError::IllegalReturnError             => write!(f, "Cannot call return outside of a function"),

            VmError::UndefinedOpcodeError{ opcode }               => write!(f, "Undefined opcode '{}' encountered", opcode),
            VmError::UndefinedImportError{ package }              => write!(f, "Undefined package '{}'", package),
            VmError::IllegalGlobalIdentifierError{ target }       => write!(f, "Illegal identifier of type {}: expected a String", target),
            VmError::UndefinedGlobalError{ identifier }           => write!(f, "Undefined global '{}'", identifier),
            VmError::UndefinedPropertyError{ instance, property } => write!(f, "Class '{}' has no property '{}' defined", instance, property),
            VmError::UndefinedMethodError{ class, method }        => write!(f, "Class '{}' has no method '{}' defined", class, method),
            VmError::IllegalServiceMethod{ method }               => write!(f, "Method '{}' is not part of the Service class", method),

            VmError::FunctionArityError{ name, got, expected } => write!(f, "Function '{}' expects {} arguments, but got {}", name, expected, got),
            VmError::ArrayArityError{ got, expected }          => write!(f, "Array expects {} values, but got {}", expected, got),
            VmError::ClassArityError{ name, got, expected }    => write!(f, "Instance of type {} requires {} properties, but got {}", name, expected, got),
            VmError::ParallelArityError{ got, expected }       => write!(f, "Parallel expects {} branches, but got {}", expected, got),

            VmError::UnsupportedPackageKindError{ name, kind } => write!(f, "Package '{}' has unsupported package kind '{}'", name, kind),
            VmError::ArrayOutOfBoundsError{ index, max }       => write!(f, "Array index {} is out-of-bounds for Array of size {}", index, max),

            VmError::DanglingHandleError => write!(f, "Encountered dangling handle on the stack"),

            VmError::CallFrameInstrError{ err }         => write!(f, "Could not read next instruction from the callframe: {}", err),
            VmError::CallFrame8bitError{ what, err }    => write!(f, "Could not read 8-bit embedded constant ({}) from the callframe: {}", what, err),
            VmError::CallFrame16bitError{ what, err }   => write!(f, "Could not read 16-bit embedded constant ({}) from the callframe: {}", what, err),
            VmError::CallFrameConstError{ what, err }   => write!(f, "Could not read a constant ({}) from the callframe: {}", what, err),
            VmError::StackReadError{ what, err }        => write!(f, "Could not read a value ({}) from the stack: {}", what, err),
            VmError::ObjectError{ err }                 => write!(f, "An error occurred while working with objects: {}", err),
            VmError::BuiltinCallError{ builtin, err }   => write!(f, "Could not perform builtin call to builtin '{}': {}", builtin, err),
            VmError::ExternalCallError{ function, err } => write!(f, "Could not perform external call to function '{}': {}", function, err),
            VmError::ClientTxError{ err }               => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for VmError {}
/*******/



#[derive(Clone, Default, Debug)]
pub struct VmOptions {
    ///
    ///
    ///
    pub clear_after_main: bool,

    ///
    ///
    ///
    pub global_return_halts: bool,
}

#[derive(Clone, Default, Debug)]
pub struct VmState {
    globals: FnvHashMap<String, Value>,
    options: VmOptions,
}

unsafe impl Send for VmState {}

impl VmState {
    fn new(
        globals: FnvHashMap<String, Value>,
        options: VmOptions,
    ) -> Self {
        Self { globals, options }
    }

    ///
    ///
    ///
    fn get_globals(
        &self,
        heap: &mut Heap<Object>,
    ) -> FnvHashMap<String, Slot> {
        let mut globals = FnvHashMap::default();

        // First process all the the classes.
        for (name, value) in &self.globals {
            if let Value::Class(_) = value {
                let slot = Slot::from_value(value.clone(), &globals, heap);
                globals.insert(name.clone(), slot);
            }
        }

        // Then the rest of the globals.
        for (name, value) in &self.globals {
            if let Value::Class(_) = value {
                continue;
            } else {
                let slot = Slot::from_value(value.clone(), &globals, heap);
                globals.insert(name.clone(), slot);
            }
        }

        globals
    }
}

///
///
///
pub struct Vm<E>
where
    E: VmExecutor + Clone + Send + Sync,
{
    executor: E,
    frames: SmallVec<[CallFrame; 64]>,
    globals: FnvHashMap<String, Slot>,
    heap: Heap<Object>,
    locations: Vec<Handle<Object>>,
    package_index: PackageIndex,
    options: VmOptions,
    stack: Stack,
}

impl<E> Default for Vm<E>
where
    E: VmExecutor + Clone + Send + Sync + Default,
{
    fn default() -> Self {
        let executor = E::default();
        let frames = SmallVec::with_capacity(64);
        let globals = FnvHashMap::<String, Slot>::with_capacity_and_hasher(256, Default::default());
        let heap = Heap::default();
        let locations = Vec::with_capacity(16);
        let package_index = PackageIndex::default();
        let options = VmOptions::default();
        let stack = Stack::default();

        Self::new(
            executor,
            frames,
            globals,
            heap,
            locations,
            package_index,
            options,
            stack,
        )
    }
}

impl<E> Vm<E>
where
    E: VmExecutor + Clone + Send + Sync,
{
    ///
    ///
    ///
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        executor: E,
        frames: SmallVec<[CallFrame; 64]>,
        globals: FnvHashMap<String, Slot>,
        heap: Heap<Object>,
        locations: Vec<Handle<Object>>,
        package_index: PackageIndex,
        options: VmOptions,
        stack: Stack,
    ) -> Self {
        let mut globals = globals;
        let mut heap = heap;

        builtins::register(&mut globals, &mut heap);

        Self {
            executor,
            frames,
            globals,
            heap,
            locations,
            package_index,
            options,
            stack,
        }
    }

    ///
    ///
    ///
    pub fn new_with(
        executor: E,
        package_index: Option<PackageIndex>,
        options: Option<VmOptions>,
    ) -> Self {
        // Override options, if provided.
        let mut state = VmState::default();
        if let Some(options) = options {
            state.options = options;
        }

        Self::new_with_state(executor, package_index, state)
    }

    ///
    ///
    ///
    pub fn new_with_state(
        executor: E,
        package_index: Option<PackageIndex>,
        state: VmState,
    ) -> Self {
        let package_index = package_index.unwrap_or_default();
        let mut heap = Heap::default();

        Self::new(
            executor,
            Default::default(),
            state.get_globals(&mut heap),
            heap,
            Default::default(),
            package_index,
            state.options,
            Stack::default(),
        )
    }

    ///
    ///
    ///
    pub fn capture_state(&self) -> VmState {
        let mut globals = FnvHashMap::default();
        for (name, slot) in &self.globals {
            let value = (*slot).into_value(&self.heap);
            globals.insert(name.clone(), value);
        }

        VmState::new(globals, self.options.clone())
    }

    /* TIM */
    /// The VM's main function, which runs the given function as main.
    ///
    /// **Arguments**
    ///  * `function`: The function to run on this VM.
    /// 
    /// **Returns**  
    /// Nothing as Ok() if it was successfull, or an Err() with the reason why it wasn't otherwise.
    pub async fn main(&mut self, function: FunctionMut) -> Result<(), VmError> {
        if !self.frames.is_empty() || !self.stack.is_empty() {
            panic!("VM not in a state to accept main function.");
        }

        let function = Object::Function(function.freeze(&mut self.heap));
        let handle = self.heap.insert(function).into_handle();

        self.stack.push_object(handle);
        if let Err(reason) = self.call(0).await { return Err(reason); }
        let res = self.run().await;

        // For REPLs
        if self.options.clear_after_main {
            self.frames.pop();
            self.stack.pop().unwrap();
        }

        // We were successfull
        return res;
    }
    /*******/

    /* TIM */
    /// Function that runs the VM with an anonymous function.
    ///
    /// **Arguments**
    ///  * `function`: The VM function to run.
    /// 
    /// **Returns**  
    /// The value of the function if it was successfull, or Err() with a reason otherwise.
    pub async fn anonymous(&mut self, function: FunctionMut) -> Result<Value, VmError> {
        if function.arity != 0 {
            panic!("Not a nullary function.");
        }

        self.options.global_return_halts = true;

        let function = Object::Function(function.freeze(&mut self.heap));
        let handle = self.heap.insert(function).into_handle();

        self.stack.push_object(handle);
        if let Err(reason) = self.call(0).await { return Err(reason); }
        if let Err(reason) = self.run().await { return Err(reason); }

        // Get the result of the stack
        if self.stack.len() == 1 {
            Ok(self.stack.pop().unwrap().into_value(&self.heap))
        } else {
            Ok(Value::Unit)
        }
    }
    /*******/

    /* TIM */
    /// **Edited: The function signature by adding the Result<(), VmError> return type and internal code to also take into account error handling.**
    ///
    /// Calls a non-root (i.e., non-main) function on the callframe stack.
    /// 
    /// **Arguments**
    ///  * `arity`: The address of the function to call(?)
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, but a VmError if it wasn't.
    async fn call(
        &mut self,
        arity: u8,
    ) -> Result<(), VmError> {
        let frame_last = self.stack.len();
        let frame_first = frame_last - (arity + 1) as usize;

        let function = self.stack.get(frame_first).as_object().expect("");
        if let Some(Object::Function(_f)) = self.heap.get(function) {
            // Debug to the client what we're going to call
            if let Err(reason) = self.executor.debug(_f.chunk.disassemble().unwrap().to_string()).await {
                let err = VmError::ClientTxError{ err: reason };
                error!("{}", &err);
                return Err(err);
            }

            // Position 0 is the main function, never allow it as root for a nested call frame.
            let frame = CallFrame::new(function, max(frame_first, 1));
            self.frames.push(frame);

            // Done
            return Ok(());
        }

        // Should never get here
        panic!("Running 'call()' on top of stack, but top of stack isn't a function; this should never happen!");
    }
    /*******/

    /* TIM */
    /// The run function, which runs instructions until there are no more available.
    ///
    /// **Returns**  
    /// Nothing if it was successfull, but if an error occurred the user should
    /// know about then it is returned as an Err.
    async fn run(&mut self) -> Result<(), VmError> {
        loop {
            // Get the next instruction
            let instruction = self.frame().read_u8();
            // Stop for the ip-out-of-bounds error, but crash for the rest
            if let Err(CallFrameError::IPOutOfBounds{ ip: _, max: _ }) = instruction { break; }
            if let Err(reason) = instruction { return Err(VmError::CallFrameInstrError{ err: reason }); }

            // Otherwise, switch on the byte we found
            match *instruction.ok().unwrap() {
                OP_ADD => self.op_add()?,
                OP_AND => self.op_and()?,
                OP_ARRAY => self.op_array()?,
                OP_CALL => self.op_call().await?,
                OP_CLASS => self.op_class()?,
                OP_CONSTANT => self.op_constant()?,
                OP_DEFINE_GLOBAL => self.op_define_global()?,
                OP_DIVIDE => self.op_divide()?,
                OP_DOT => self.op_dot()?,
                OP_EQUAL => self.op_equal()?,
                OP_FALSE => self.op_false(),
                OP_GET_GLOBAL => self.op_get_global()?,
                OP_GET_LOCAL => self.op_get_local()?,
                OP_GET_METHOD => self.op_get_method()?,
                OP_GET_PROPERTY => self.op_get_property()?,
                OP_GREATER => self.op_greater()?,
                OP_IMPORT => self.op_import().await?,
                OP_INDEX => self.op_index()?,
                OP_JUMP => self.op_jump()?,
                OP_JUMP_BACK => self.op_jump_back()?,
                OP_JUMP_IF_FALSE => self.op_jump_if_false()?,
                OP_LESS => self.op_less()?,
                OP_LOC => self.op_loc(),
                OP_LOC_POP => self.op_loc_pop(),
                OP_LOC_PUSH => self.op_loc_push()?,
                OP_MULTIPLY => self.op_multiply()?,
                OP_NEGATE => self.op_negate()?,
                OP_NEW => self.op_new()?,
                OP_NOT => self.op_not()?,
                OP_OR => self.op_or()?,
                OP_PARALLEL => self.op_parallel()?,
                OP_POP => self.op_pop()?,
                OP_POP_N => self.op_pop_n()?,
                OP_RETURN => {
                    self.op_return()?;
                    // Stop if that was the last frame
                    if self.options.global_return_halts && self.frames.is_empty() {
                        break;
                    }
                }
                OP_SET_GLOBAL => self.op_set_global(false)?,
                OP_SET_LOCAL => self.op_set_local()?,
                OP_SUBSTRACT => self.op_substract()?,
                OP_TRUE => self.op_true(),
                OP_UNIT => self.op_unit(),
                x => {
                    return Err(VmError::UndefinedOpcodeError{ opcode: x });
                }
            }

            // INVESTIGATE: this appears to cause a deadlock (?).
            // debug!("Sending stack to client.");
            // self.executor.debug(format!("{}", self.stack)).await.unwrap();
            // debug!("Sent stack to client.");
        }

        debug!("No more instructions to process within this call frame.");

        // We did everything well
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError.**
    ///
    /// Returns the 'arity' topmost values on the stack as arguments for a function.
    /// 
    /// **Returns**  
    /// A vector with the arguments as Values if the call went alright, or a the number of arguments we got instead if it failed.
    fn arguments(&mut self, arity: u8) -> Result<Vec<Value>, u8> {
        // let mut arguments: Vec<Value> = (0..arity).map(|_| self.stack.pop().into_value(&self.heap)).collect();
        let mut arguments: Vec<Value> = Vec::new();
        for i in 0..arity {
            // Try to pop the top value
            let val = self.stack.pop();
            if let Err(_) = val { return Err(i); }
            
            // Add it to the list
            arguments.push(val.unwrap().into_value(&self.heap));
        }

        // Reverse the arguments, then return
        arguments.reverse();
        Ok(arguments)
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    fn frame(&mut self) -> &mut CallFrame {
        self.frames.last_mut().expect("No frame in VM; this should never happen!")
    }

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    /// 
    /// Performs the add-operation on the two topmost values on the stack.
    /// 
    /// **Returns**  
    /// Nothing if the call was alright, but an Err(VmError) if it couldn't be completed somehow.
    #[inline]
    pub fn op_add(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a numeric value or a string".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a numeric value or a string".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Switch on the values
        match (lhs, rhs) {
            (Slot::Integer(lhs), Slot::Integer(rhs)) => self.stack.push_integer(lhs + rhs),
            (Slot::Integer(lhs), Slot::Real(rhs))    => self.stack.push_real(lhs as f64 + rhs),
            (Slot::Real(lhs), Slot::Real(rhs))       => self.stack.push_real(lhs + rhs),
            (Slot::Real(lhs), Slot::Integer(rhs))    => self.stack.push_real(lhs + rhs as f64),
            (Slot::Object(lsh), Slot::Object(rhs))   => {
                // Re-interpret the lefthandisde a string
                let lhs = self.heap.get(lsh);
                if let None = lhs { return Err(VmError::DanglingHandleError); }
                let slhs = lhs.unwrap().as_string();
                // Also do the righthandside
                let rhs = self.heap.get(rhs);
                if let None = rhs { return Err(VmError::DanglingHandleError); }
                let srhs = rhs.unwrap().as_string();

                // Check if they are indeed strings
                match (slhs, srhs) {
                    (Some(lhs), Some(rhs)) => {
                        let mut new = lhs.clone();
                        new.push_str(rhs);

                        let object = self.heap.insert(Object::String(new));
                        let object = object.into_handle();

                        self.stack.push_object(object);
                    }
                    _ => { return Err(VmError::NotAddable{ lhs: lhs.unwrap().data_type(), rhs: rhs.unwrap().data_type() }); },
                }
            },
            _ => { return Err(VmError::NotAddable{ lhs: lhs.data_type(), rhs: rhs.data_type() }); }
        };

        // Done
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    ///
    /// Performs the logical-and operation on the two topmost values on the stack.
    /// 
    /// **Returns**  
    /// Nothing if the call was alright, but an Err(VmError) if it couldn't be completed somehow.
    #[inline]
    pub fn op_and(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop_boolean();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a boolean value".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop_boolean();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a boolean value".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Add the result of the and-operation to the stack
        self.stack.push_boolean(lhs && rhs);
        Ok(())
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    pub fn op_array(&mut self) -> Result<(), VmError> {
        let n = *self.frame().read_u8().expect("");
        let mut elements: Vec<Slot> = Vec::new();
        for i in 0..n {
            // Try to get the stack value
            let val = self.stack.pop();
            if let Err(_) = val { return Err(VmError::ArrayArityError{ got: i, expected: n }); }

            // Add it to the list
            elements.push(val.unwrap());
        }
        elements.reverse();

        /* TIM */
        // let mut array = Object::Array(Array::new(elements));
        let mut raw_array = Array::new(elements);
        if let Err(reason) = raw_array.resolve_type(&self.heap) { return Err(VmError::ObjectError{ err: reason }); }
        let array = Object::Array(raw_array);
        /*******/
        let handle = self.heap.insert(array).into_handle();

        self.stack.push(Slot::Object(handle));

        // Done
        Ok(())
    }

    /* TIM */
    /// **Edited: now returning errors from buildins (see builtins.rs), local functions and external functions; also edited doc comment**
    ///
    /// Performs an OP_CALL, which call either some builtin, local function or external function that has to be called with the framework.
    /// 
    /// **Returns**  
    /// Nothing if the call was alright, but an Err(VmError) if it couldn't be completed somehow.
    #[inline]
    pub async fn op_call(&mut self) -> Result<(), VmError> {
        // Get the arity of the callframe (i.e., the number of arguments)
        let arity = self.frame().read_u8();
        if let Err(reason) = arity { return Err(VmError::CallFrame8bitError{ what: "an arity".to_string(), err: reason }); }
        let arity = *arity.unwrap();

        // Get the boundries of this frame
        let frame_last = self.stack.len();
        let frame_first = frame_last - (arity + 1) as usize;

        // Get the function pointer
        let function = self.stack.get(frame_first);
        let location = self
            .locations
            .last()
            .map(|l| self.heap.get(l).unwrap())
            .map(|l| l.as_string().cloned().unwrap());

        // Determine how to call
        let value = match function {
            Slot::BuiltIn(code) => {
                // Get the builtin call and its arguments
                let function = *code;
                let arguments = self.arguments(arity);
                if let Err(i) = arguments { return Err(VmError::FunctionArityError{ name: format!("{}", function), got: i, expected: arity }); }

                // Do the call
                let res = builtins::call(function, arguments.unwrap(), &self.executor, location).await;
                if let Err(reason) = res {
                    // Do an early debug print
                    let err = VmError::BuiltinCallError{ builtin: function, err: reason };
                    debug!("{}", &err);
                    return Err(err);
                }
                res.ok().unwrap()
            }
            Slot::Object(handle) => match self.heap.get(handle).expect("") {
                Object::Function(_) => {
                    // Execution is handled through call frames.
                    let res = self.call(arity).await;
                    if let Err(reason) = res {
                        // Do an early debug print
                        debug!("Failed to call local function: {}", &reason);
                        return Err(reason);
                    }
                    // Return early, since we're not interested in this function's return value (apparently)
                    return Ok(());
                }
                Object::FunctionExt(f) => {
                    // Get the function and its arguments
                    let function = f.clone();
                    let arguments = self.arguments(arity);
                    if let Err(i) = arguments { return Err(VmError::FunctionArityError{ name: function.name.clone(), got: i, expected: arity }); }

                    // Map the arguments to key/value pairs
                    let arguments = itertools::zip(&function.parameters, arguments.unwrap())
                        .map(|(p, a)| (p.name.clone(), a))
                        .collect();

                    // Do the call
                    let function_name = function.name.clone();
                    match self.executor.call(function, arguments, location).await {
                        Ok(value) => {
                            debug!("Value from function '{}' (external): \n{:#?}", function_name, value);
                            value
                        }
                        Err(reason) => {
                            // Do an early debug print
                            let err = VmError::ExternalCallError{ function: function_name, err: reason };
                            debug!("{}", &err);
                            return Err(err);
                        }
                    }
                }
                object => {
                    dbg!(&object);
                    dbg!(&self.stack);
                    panic!("Not a callable object");
                }
            },
            _ => panic!("Not a callable object"),
        };

        // Remove (built-in or external) function from the stack.
        self.stack.pop().unwrap();

        // Store return value on the stack.
        let slot = Slot::from_value(value, &self.globals, &mut self.heap);
        self.stack.push(slot);

        debug!("Completed call to op_call.");
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Pushes a class from the callframe onto the stack.
    /// 
    /// **Returns**  
    /// Nothing if everything went fine, or a VmError otherwise.
    #[inline]
    pub fn op_class(&mut self) -> Result<(), VmError> {
        // Try to read the class from the frame
        let res = self.frame().read_constant();
        if let Err(reason) = res { return Err(VmError::CallFrameConstError{ what: "a class".to_string(), err: reason }); }
        let class = *res.unwrap();

        // Push it onto the stack
        self.stack.push(class);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Pushes a constant from the callframe onto the stack.
    /// 
    /// **Returns**  
    /// Nothing if everything went fine, or a VmError otherwise.
    #[inline]
    pub fn op_constant(&mut self) -> Result<(), VmError> {
        // Try to read the constant from the frame
        let res = self.frame().read_constant();
        if let Err(reason) = res { return Err(VmError::CallFrameConstError{ what: "a constant".to_string(), err: reason }); }
        let constant = *res.unwrap();

        // Push it onto the stack
        self.stack.push(constant);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors because called function does too.**
    ///
    /// Defines a new global function.
    /// 
    /// **Returns**  
    /// Nothing if everything went fine, or a VmError otherwise.
    #[inline]
    pub fn op_define_global(&mut self) -> Result<(), VmError> {
        self.op_set_global(true)
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Performs a division on the two most recent values on the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_divide(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Do the division based on what is given to us
        // TODO: Talk about integer VS float division in the documentation.
        match (lhs, rhs) {
            (Slot::Integer(lhs), Slot::Integer(rhs)) => self.stack.push_integer(lhs / rhs),
            (Slot::Integer(lhs), Slot::Real(rhs))    => self.stack.push_real(lhs as f64 / rhs),
            (Slot::Real(lhs), Slot::Real(rhs))       => self.stack.push_real(lhs / rhs),
            (Slot::Real(lhs), Slot::Integer(rhs))    => self.stack.push_real(lhs / rhs as f64),
            _                                        => { return Err(VmError::NotDivisible{ lhs: lhs.into_value(&self.heap).data_type(), rhs: rhs.into_value(&self.heap).data_type() }) },
        };

        // Done
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Applies the dot-operator to the last object on the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_dot(&mut self) -> Result<(), VmError> {
        // Try to get the object to work on
        let slot = self.stack.pop();
        if let Err(reason) = slot { return Err(VmError::StackReadError{ what: "an instance".to_string(), err: reason }); }
        let slot = slot.unwrap();
        let object = slot.as_object();
        if let None = object { return Err(VmError::IllegalDotError{ target: slot.into_value(&self.heap).data_type() }); }
        let object = object.unwrap();

        // Read the property which we use to access from the callframe
        let res = self.frame().read_constant();
        if let Err(reason) = res { return Err(VmError::CallFrameConstError{ what: "a property".to_string(), err: reason }); }
        let res = res.unwrap();
        let property = res.as_object();
        if let None = property { return Err(VmError::IllegalPropertyError{ target: res.into_value(&self.heap).data_type() }); }
        let property = property.unwrap();

        // Next, try if the object points to an Instance on the heap
        if let Some(object) = self.heap.get(object) {
            if let Object::Instance(instance) = object {
                // Now check if the property points to a string on the heap
                if let Some(property) = self.heap.get(property) {
                    if let Object::String(property) = property {
                        // They both do, so finally check if the instance has that property
                        let value = instance.properties.get(property);
                        if let None = value { return Err(VmError::UndefinedPropertyError{ instance: format!("{}", &instance), property: property.clone() }); }
                        let value = *value.unwrap();

                        // Finally, push the value of that property on the stack
                        self.stack.push(value);

                        // Done!
                        return Ok(());
                    } else {
                        return Err(VmError::IllegalPropertyError{ target: property.data_type() });
                    }
                } else {
                    return Err(VmError::DanglingHandleError);
                }
            } else {
                return Err(VmError::IllegalDotError{ target: object.data_type() })
            }
        } else {
            return Err(VmError::DanglingHandleError);
        }
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    /// 
    /// Tests whether the top two values on the stack are the same.
    /// 
    /// **Returns**  
    /// Nothing if the call was alright, but an Err(VmError) if it couldn't be completed somehow.
    #[inline]
    pub fn op_equal(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "anything".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "anything".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Push the result of the comparison
        self.stack.push_boolean(lhs == rhs);
        Ok(())
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    pub fn op_false(&mut self) {
        self.stack.push(Slot::False);
    }

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Tries to get the value of a global.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_get_global(&mut self) -> Result<(), VmError> {
        // Try to get the global's identifier
        let identifier = self.frame().read_constant();
        if let Err(reason) = identifier { return Err(VmError::CallFrameConstError{ what: "a global identifier".to_string(), err: reason }); }
        let identifier = *identifier.unwrap();

        // See if the identifier is a string
        if let Slot::Object(handle) = identifier {
            if let Some(identifier) = self.heap.get(handle) {
                if let Object::String(identifier) = identifier {
                    // Get the matching global
                    let value = self.globals.get(identifier);
                    if let None = value { return Err(VmError::UndefinedGlobalError{ identifier: identifier.clone() }); }
                    let value = *value.unwrap();

                    // Push its value onto the stack
                    self.stack.push(value);

                    // Done
                    return Ok(());
                } else {
                    return Err(VmError::IllegalGlobalIdentifierError{ target: identifier.data_type() });
                }
            } else {
                return Err(VmError::DanglingHandleError);
            }
        } else {
            return Err(VmError::IllegalGlobalIdentifierError{ target: identifier.into_value(&self.heap).data_type() });
        }
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Tries to get the value of a local variable.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_get_local(&mut self) -> Result<(), VmError> {
        // Get the index of the local variable on the stack
        let frame = self.frame();
        let index = frame.read_u8();
        if let Err(reason) = index { return Err(VmError::CallFrame8bitError{ what: "a local offset".to_string(), err: reason }); }
        let index = *index.unwrap() as usize;

        // Get the matching variable and push it onto the stack
        let index = frame.stack_offset + index;
        self.stack.copy_push(index);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors and fixed opcodes to be with Builtin-enum instead.**
    ///
    /// Prepares calling a method by reserving its identifier and checking its instance.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_get_method(&mut self) -> Result<(), VmError> {
        // Check if we're calling on an instance
        let instance_slot = self.stack.pop();
        if let Err(reason) = instance_slot { return Err(VmError::StackReadError{ what: "an instance".to_string(), err: reason }); }
        let instance_slot = instance_slot.unwrap();
        let instance = instance_slot.as_object();
        if let None = instance { return Err(VmError::MethodDotError{ target: instance_slot.into_value(&self.heap).data_type() }); }
        let instance = instance.unwrap();

        // Try to get the method
        let method = self.frame().read_constant();
        if let Err(reason) = method { return Err(VmError::CallFrameConstError{ what: "a method name".to_string(), err: reason }); }
        let method = method.unwrap();
        let method_handle = method.as_object();
        if let None = method_handle { return Err(VmError::IllegalPropertyError{ target: method.into_value(&self.heap).data_type() }); }
        let method_handle = method_handle.unwrap();

        // Next, try if the object points to an Instance on the heap
        if let Some(instance) = self.heap.get(instance) {
            if let Object::Instance(instance) = instance {
                // From instance, we move on to try to get the method string
                if let Some(method) = self.heap.get(method_handle) {
                    if let Object::String(method) = method {
                        // Then, we try to obtain the class behind the instance
                        if let Some(class) = self.heap.get(instance.class) {
                            if let Object::Class(class) = class {
                                // Now we have everything, determine if we launch the function synchronously or asynchronously
                                let method = if class.name == *"Service" {
                                    // We launch it asynchronously, so wrap in the builtins
                                    match method.as_str() {
                                        // Quickfix :(
                                        "waitUntilStarted" => Slot::BuiltIn(BuiltinFunction::WaitUntilStarted),
                                        "waitUntilDone" => Slot::BuiltIn(BuiltinFunction::WaitUntilDone),
                                        _ => { return Err(VmError::IllegalServiceMethod{ method: method.clone() }); }
                                    }
                                } else {
                                    // Simply get the method as normal
                                    let real_method = class.methods.get(method);
                                    if let None = real_method { return Err(VmError::UndefinedMethodError{ class: class.name.clone(), method: method.clone() }); }
                                    *real_method.unwrap()
                                };

                                // With the proper method chosen, write it and the instance to the stack
                                self.stack.push(method);
                                self.stack.push(instance_slot);

                                // Done!
                                return Ok(());
                            } else {
                                panic!("Instance does not have a Class as baseclass, but a {} ('{}') instead; this should never happen!", class.data_type(), class);
                            }
                        } else {
                            return Err(VmError::DanglingHandleError);
                        }
                    } else {
                        return Err(VmError::IllegalPropertyError{ target: method.data_type() })
                    }
                } else {
                    return Err(VmError::DanglingHandleError);
                }
            } else {
                return Err(VmError::MethodDotError{ target: instance.data_type() })
            }
        } else {
            return Err(VmError::DanglingHandleError);
        }
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Returns the given property from the object on the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_get_property(&mut self) -> Result<(), VmError> {
        // Try to get the instance
        let instance_slot = self.stack.pop();
        if let Err(reason) = instance_slot { return Err(VmError::StackReadError{ what: "an instance".to_string(), err: reason }); }
        let instance_slot = instance_slot.unwrap();
        let instance = instance_slot.as_object();
        if let None = instance { return Err(VmError::IllegalDotError{ target: instance_slot.into_value(&self.heap).data_type() }); }
        let instance = instance.unwrap();

        // Get the property from the frame
        let property = self.frame().read_constant();
        if let Err(reason) = property { return Err(VmError::CallFrameConstError{ what: "an instance property".to_string(), err: reason }); }
        let property = property.unwrap();
        let property_handle = property.as_object();
        if let None = property_handle { return Err(VmError::IllegalPropertyError{ target: property.into_value(&self.heap).data_type() }); }
        let property_handle = property_handle.unwrap();

        // Now check if the object is actually an instance
        if let Some(instance) = self.heap.get(instance) {
            if let Object::Instance(instance) = instance {
                // Next, check if the property points to a string
                if let Some(property) = self.heap.get(property_handle) {
                    if let Object::String(property) = property {
                        // Check if the instance actually has this property
                        let value = instance.properties.get(property);
                        if let None = value { return Err(VmError::UndefinedPropertyError{ instance: format!("{}", &instance), property: property.clone() }); }
                        let value = *value.unwrap();

                        // Push the property's value onto the stack
                        self.stack.push(value);
                        return Ok(());
                    } else {
                        return Err(VmError::IllegalPropertyError{ target: property.data_type() })
                    }
                } else {
                    return Err(VmError::DanglingHandleError);
                }
            } else {
                return Err(VmError::IllegalDotError{ target: instance.data_type() })
            }
        } else {
            return Err(VmError::DanglingHandleError);
        }
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    /// 
    /// Compares the top two values on the stack in terms of being greater than the other.
    /// 
    /// **Returns**  
    /// Nothing if the call was alright, but an Err(VmError) if it couldn't be completed somehow.
    #[inline]
    pub fn op_greater(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Run the comparison
        let value = match (rhs, lhs) {
            (Slot::Integer(rhs), Slot::Integer(lhs)) => rhs > lhs,
            (Slot::Integer(rhs), Slot::Real(lhs)   ) => (rhs as f64) > lhs,
            (Slot::Real(rhs),    Slot::Integer(lhs)) => rhs > (lhs as f64),
            (Slot::Real(rhs),    Slot::Real(lhs)   ) => rhs > lhs,
            (rhs, lhs)                               => { return Err(VmError::NotComparable{ rhs: rhs.data_type(), lhs: lhs.data_type() }); }
        };

        // Push the result on the stack
        self.stack.push_boolean(value);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now supports returning VmErrors instead of panicking.**
    ///
    /// Tries to import a given package.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub async fn op_import(&mut self) -> Result<(), VmError> {
        // Get the import name first
        let p_name = self.frame().read_constant();
        if let Err(reason) = p_name { return Err(VmError::CallFrameConstError{ what: "an package identifier".to_string(), err: reason }); }
        let p_name = p_name.unwrap();
        let p_name_handle = p_name.as_object();
        if let None = p_name_handle { return Err(VmError::IllegalImportError{ target: p_name.into_value(&self.heap).data_type() }); }
        let p_name_handle = p_name_handle.unwrap();

        // Try to get the string behind the handle
        if let Some(p_name) = self.heap.get(p_name_handle) {
            if let Object::String(p_name) = p_name {
                // Try to get the package from the list
                let p_name = p_name.clone();
                let package = self.package_index.get(&p_name, None);
                if let None = package { return Err(VmError::UndefinedImportError{ package: p_name }); }
                let package = package.unwrap();

                // Resolve the package type
                // TODO: update upstream so we don't need this anymore.
                let kind = match package.kind.as_str() {
                    "ecu" => String::from("code"),
                    "oas" => String::from("oas"),
                    _ => return Err(VmError::UnsupportedPackageKindError{ name: p_name, kind: package.kind.clone() }),
                };

                // Try to resolve the list of functions behind the package
                if let Some(functions) = &package.functions {
                    // Create a function handle for each of them in the list of globals
                    // Also collect a string representation of the list to show to the user
                    let mut sfunctions = String::new();
                    for (f_name, function) in functions {
                        // Create the FunctionExt handle
                        let function = FunctionExt {
                            name: f_name.clone(),
                            detached: package.detached,
                            package: p_name.clone(),
                            kind: kind.clone(),
                            version: package.version.clone(),
                            parameters: function.parameters.clone(),
                        };

                        // Write it to the heap
                        let handle = self.heap.insert(Object::FunctionExt(function)).into_handle();
                        let object = Slot::Object(handle);

                        // Insert the global
                        self.globals.insert(f_name.clone(), object);

                        // Update the list of functions
                        if sfunctions.len() > 0 { sfunctions += ", "; }
                        sfunctions += &format!("'{}'", f_name.clone());
                    }

                    // Let the user know how many we imported
                    if let Err(reason) = self.executor.debug(format!("Package '{}' provides {} functions: {}", p_name, functions.len(), sfunctions)).await {
                        error!("Could not send debug message to client: {}", reason);
                    };
                } else {
                    if let Err(reason) = self.executor.debug(format!("Package '{}' provides no functions", p_name)).await {
                        error!("Could not send debug message to client: {}", reason);
                    };
                }

                // Next, import the types provided by the package
                if let Some(types) = &package.types {
                    // Go through the types, constructing a list of them as we go
                    let mut stypes = String::new();
                    for t_name in types.keys() {
                        // Create the Class handle
                        let class = Class {
                            name: t_name.clone(),
                            methods: Default::default(),
                        };

                        // Write it to the heap
                        let handle = self.heap.insert(Object::Class(class)).into_handle();
                        let object = Slot::Object(handle);

                        // Insert the global
                        self.globals.insert(t_name.clone(), object);

                        // Update the list of types
                        if stypes.len() > 0 { stypes += ", "; }
                        stypes += &format!("'{}'", t_name.clone());
                    }

                    // Let the user know how many we imported
                    if let Err(reason) = self.executor.debug(format!("Package '{}' provides {} custom types: {}", p_name, types.len(), stypes)).await {
                        error!("Could not send debug message to client: {}", reason);
                    };
                } else {
                    if let Err(reason) = self.executor.debug(format!("Package '{}' provides no custom types", p_name)).await {
                        error!("Could not send debug message to client: {}", reason);
                    };
                }

                // Done!
                if let Err(reason) = self.executor.debug(format!("Imported package '{}' successfully", p_name)).await {
                    error!("Could not send debug message to client: {}", reason);
                };
                return Ok(())
            } else {
                return Err(VmError::IllegalImportError{ target: p_name.data_type() });
            }
        } else {
            return Err(VmError::DanglingHandleError);
        }
    }
    /*******/

    /* TIM */
    /// **Edited: now supports returning VmErrors instead of panicking.**
    ///
    /// Indexes the given Array and returns its value at that location on the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_index(&mut self) -> Result<(), VmError> {
        // Get the index from the stack
        let index = self.stack.pop_integer();
        if let Err(reason) = index { return Err(VmError::StackReadError{ what: "an array index".to_string(), err: reason }); }
        let index = index.unwrap();

        // Get the array object from the stack
        let array = self.stack.pop_object();
        if let Err(reason) = array { return Err(VmError::StackReadError{ what: "an array handle".to_string(), err: reason }); }

        // Try to get the Array behind the stack object
        if let Some(array) = self.heap.get(array.unwrap()) {
            if let Object::Array(array) = array {
                // Try to get the element from the array
                if let Some(element) = array.elements.get(index as usize) {
                    // Put the value on the stack
                    self.stack.push(*element);
                    return Ok(());
                } else {
                    return Err(VmError::ArrayOutOfBoundsError{ index: index as usize, max: array.elements.len() });
                }
            } else {
                return Err(VmError::IllegalIndexError{ target: array.data_type() });
            }
        } else {
            return Err(VmError::DanglingHandleError);
        }
    }
    /*******/

    /* TIM */
    /// **Edited: now supports returning VmErrors instead of panicking.**
    /// 
    /// Performs a forward jump based on the embedded 16-bit constant in the function code.
    ///
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_jump(&mut self) -> Result<(), VmError> {
        let offset = self.frame().read_u16();
        if let Err(reason) = offset { return Err(VmError::CallFrame16bitError{ what: "a jump offset".to_string(), err: reason }); }
        self.frame().ip += offset.unwrap() as usize;
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now supports returning VmErrors instead of panicking.**
    /// 
    /// Performs a backward jump based on the embedded 16-bit constant in the function code.
    ///
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_jump_back(&mut self) -> Result<(), VmError> {
        let offset = self.frame().read_u16();
        if let Err(reason) = offset { return Err(VmError::CallFrame16bitError{ what: "a jump offset".to_string(), err: reason }); }
        self.frame().ip -= offset.unwrap() as usize;
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now supports returning VmErrors instead of panicking.**
    ///
    /// Performs a forward jump if the top of the stack is false.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_jump_if_false(&mut self) -> Result<(), VmError> {
        // Get the top value
        // TODO: Is it correct that this is a peek?
        let truthy = self.stack.peek_boolean();
        if let Err(reason) = truthy { return Err(VmError::StackReadError{ what: "a jump value".to_string(), err: reason }); }

        // Switch on it
        if !truthy.unwrap() {
            // It's a false so jump
            return self.op_jump();
        }

        // Skip the next two bytes detailling the offset
        self.frame().ip += 2;
        return Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now supports returning VmErrors instead of panicking.**
    ///
    /// Compares the two top values on the stack if they're numerical.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_less(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Run the comparison
        let value = match (rhs, lhs) {
            (Slot::Integer(rhs), Slot::Integer(lhs)) => rhs < lhs,
            (Slot::Integer(rhs), Slot::Real(lhs)   ) => (rhs as f64) < lhs,
            (Slot::Real(rhs),    Slot::Integer(lhs)) => rhs < (lhs as f64),
            (Slot::Real(rhs),    Slot::Real(lhs)   ) => rhs < lhs,
            (rhs, lhs)                               => { return Err(VmError::NotComparable{ rhs: rhs.data_type(), lhs: lhs.data_type() }); }
        };

        // Push the result of the comparison on the stack
        self.stack.push_boolean(value);

        // Done
        Ok(())
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    pub fn op_loc(&mut self) {
        let location = self.locations.pop().map(Slot::Object).unwrap_or(Slot::Unit);

        self.stack.push(location);
    }

    ///
    ///
    ///
    #[inline]
    pub fn op_loc_pop(&mut self) {
        self.locations.pop();
    }

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    ///
    /// Pushes the location that is on top of the stack to the location list.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_loc_push(&mut self) -> Result<(), VmError> {
        // Try to pop the location
        let location = self.stack.pop_object();
        if let Err(reason) = location { return Err(VmError::StackReadError{ what: "a location object".to_string(), err: reason }); }

        // Push the location
        self.locations.push(location.unwrap());
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Performs a multiplication on the two most recent values on the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_multiply(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Do the division based on what is given to us
        // TODO: Talk about integer VS float division in the documentation.
        match (lhs, rhs) {
            (Slot::Integer(lhs), Slot::Integer(rhs)) => self.stack.push_integer(lhs * rhs),
            (Slot::Integer(lhs), Slot::Real(rhs))    => self.stack.push_real(lhs as f64 * rhs),
            (Slot::Real(lhs), Slot::Real(rhs))       => self.stack.push_real(lhs * rhs),
            (Slot::Real(lhs), Slot::Integer(rhs))    => self.stack.push_real(lhs * rhs as f64),
            _                                        => { return Err(VmError::NotMultiplicable{ lhs: lhs.into_value(&self.heap).data_type(), rhs: rhs.into_value(&self.heap).data_type() }) },
        };

        // Done
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    ///
    /// Negates the top value on the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_negate(&mut self) -> Result<(), VmError> {
        // Get the value to negate
        let value = self.stack.pop();
        if let Err(reason) = value { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let value = value.unwrap();

        // Match the value
        let value = match value {
            Slot::Integer(i) => Slot::Integer(-i),
            Slot::Real(r)    => Slot::Real(-r),
            _                => { return Err(VmError::NotNegatable{ target: value.data_type() }); }
        };

        // Push the value on the stack
        self.stack.push(value);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    ///
    /// Pushes a new instance of the class on the stack, to the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_new(&mut self) -> Result<(), VmError> {
        // Get the number of properties for this class from the callframe
        let properties_n = self.frame().read_u8();
        if let Err(reason) = properties_n { return Err(VmError::CallFrame8bitError{ what: "number of properties".to_string(), err: reason }); }
        let properties_n = *properties_n.unwrap();

        // Get the class from the stack
        let class = self.stack.pop_object();
        if let Err(reason) = class { return Err(VmError::StackReadError{ what: "a class".to_string(), err: reason }); }
        let class_handle = class.unwrap();

        // Try to resolve the class already
        let class_obj = self.heap.get(class_handle);
        if let None = class_obj { return Err(VmError::DanglingHandleError); }
        let class_obj = class_obj.unwrap();
        let class_name: &str;
        if let Object::Class(class) = class_obj {
            class_name = &class.name;
        } else {
            return Err(VmError::IllegalNewError{ target: class_obj.data_type() });
        }

        // Get the properties themselves from the stack
        let mut properties: FnvHashMap<String, Slot> = FnvHashMap::default();
        for i in 0..properties_n {
            // Get the property name
            let key = self.stack.pop();
            if let Err(_) = key { return Err(VmError::ClassArityError{ name: class_name.to_string(), got: i, expected: properties_n }); }
            let key = key.unwrap();
            let key_handle = key.as_object();
            if let None = key_handle { return Err(VmError::IllegalPropertyError{ target: key.into_value(&self.heap).data_type() }); }
            // Get the property value
            let val = self.stack.pop();
            if let Err(reason) = val { return Err(VmError::StackReadError{ what: "a property value".to_string(), err: reason }); }

            // Try if the key is a string
            if let Some(key) = self.heap.get(key_handle.unwrap()) {
                if let Object::String(key) = key {
                    // Insert the key/value pair
                    properties.insert(key.clone(), val.unwrap());
                } else {
                    return Err(VmError::IllegalPropertyError{ target: key.data_type() });
                }
            } else {
                return Err(VmError::DanglingHandleError);
            }
        }

        // Get the class behind the handle
        if let Object::Class(_) = class_obj {
            // Create a new instance from it
            let instance = Instance::new(class_handle, properties);
            let instance = self.heap.insert(Object::Instance(instance)).into_handle();

            // Put the instance on the stack
            self.stack.push_object(instance);
            Ok(())
        } else {
            Err(VmError::IllegalNewError{ target: class_obj.data_type() })
        }
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    ///
    /// Performs the logical not-operation on the top element of the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_not(&mut self) -> Result<(), VmError> {
        // Try to get the top value as a boolean
        let value = self.stack.pop_boolean();
        if let Err(reason) = value { return Err(VmError::StackReadError{ what: "a boolean".to_string(), err: reason }); }

        // Push the reverse of the boolean on the stack
        self.stack.push_boolean(!value.unwrap());
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    ///
    /// Performs logical-or on the top two elements of the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_or(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop_boolean();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a boolean".to_string(), err: reason }); }
        // Get the lefthand side next
        let lhs = self.stack.pop_boolean();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a boolean".to_string(), err: reason }); }

        // Push the result onto the stack
        self.stack.push_boolean(lhs.unwrap() || rhs.unwrap());
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: working with the new StackError, so also returning VmErrors to accomodate that now.**
    ///
    /// Launches jobs for multiple functions at the same time.
    ///
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_parallel(&mut self) -> Result<(), VmError> {
        Err(VmError::ParallelNotImplementedError)
        // // Get the number of branches
        // let branches_n = self.frame().read_u8();
        // if let Err(reason) = branches_n { return Err(VmError::CallFrame8bitError{ what: "number of branches".to_string(), err: reason }); }
        // let branches_n = *branches_n.unwrap();

        // // Collect the branches to run
        // let mut branches: Vec<FunctionMut> = Vec::new();
        // // TODO: combine op_parallel with op_array.
        // for i in 0..branches_n {
        //     // Get the function to run from the stack
        //     let handle = self.stack.pop_object();
        //     if let Err(reason) = handle { return Err(VmError::StackReadError{ what: "a parallel branch (function)".to_string(), err: reason }); }

        //     // Get the function behind the handle
        //     let function_obj = self.heap.get(handle.unwrap());
        //     if let None = function_obj { return Err(VmError::DanglingHandleError); }
        //     let function = function_obj.unwrap().as_function();
        //     if let None = function { return Err(VmError::IllegalBranchError{ target: function_obj.unwrap().data_type() }); }
        //     let function = function.unwrap().clone();

        //     // Unfreeze the function from the heap, then push it to the list of functions to run
        //     let function = function.unfreeze(&self.heap);
        //     branches.push(function);
        // }

        // // Collects the results of the branches
        // let results = if !branches.is_empty() {
        //     // Spawn them
        //     let parresults: Vec<Value> = Vec::new();
        //     let mut threads: Vec<std::thread::JoinHandle<Result<Value, VmError>>> = Vec::new();
        //     for branch in branches {
        //         // Provide a copy of the necessary parts of the VM for the parallel job
        //         let executor = self.executor.clone();
        //         let package_index = self.package_index.clone();
        //         let state = self.capture_state();

        //         // Spawn a new thread
        //         threads.push(std::thread::spawn(move || -> Result<Value, VmError> {
        //             // Create a new VM with the same state
        //             let mut vm = Vm::<E>::new_with_state(executor, Some(package_index), state);

        //             // Wait for the VM to be done
        //             let rt = Runtime::new().unwrap();
        //             rt.block_on(vm.anonymous(branch))
        //         }));
        //     }

        //     let parresults = branches
        //         .into_par_iter()
        //         .map(|f| {
        //             let mut vm = Vm::<E>::new_with_state(executor.clone(), Some(package_index.clone()), state.clone());

        //             // TEMP: needed because the VM is not completely `send`.
        //             let rt = Runtime::new().unwrap();
        //             rt.block_on(vm.anonymous(f))
        //         })
        //         .collect::<Vec<_>>();
        //     let mut results = Vec::new();
        //     for res in parresults {
        //         match res {
        //             Ok(v) => results.push(Slot::from_value(v, &self.globals, &mut self.heap)),
        //             Err(reason) => {
        //                 eprintln!("{}", reason);
        //                 // Stop prematurely
        //                 /* TODO: Return the reason. */
        //                 return;
        //             }
        //         }
        //     }
        //     /*******/

        //     Array::new(results)
        // } else {
        //     // No branches == no results
        //     Array::new(vec![])
        // };

        // let array = Object::Array(results);
        // let array = self.heap.insert(array).into_handle();

        // self.stack.push_object(array);
    }
    /*******/

    /* TIM */
    /// **Edited: commented out the whole function for now, because I don't think it's quite thread-safe (depends on the implementation of the Heap).**
    ///
    /// Pops the last value of the stack.
    ///
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_pop(&mut self) -> Result<(), VmError> {
        let val = self.stack.pop();
        if let Err(reason) = val { return Err(VmError::StackReadError{ what: "an ignored value".to_string(), err: reason }); }
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Pops the top N values of the stack.
    ///
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_pop_n(&mut self) -> Result<(), VmError> {
        // Read from where to clear
        let x = self.frame().read_u8();
        if let Err(reason) = x { return Err(VmError::CallFrame8bitError{ what: "number of stack items to pop".to_string(), err: reason }); }
        let x = *x.unwrap() as usize;

        // Compute the index where to delete from
        let index;
        if self.stack.len() >= x { index = self.stack.len() - x; }
        else { index = 0; }

        // Do the removal, and we're done!
        self.stack.clear_from(index);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Returns from the current callframe to the one above that.
    ///
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_return(&mut self) -> Result<(), VmError> {
        // Check if we actually have something to go back to
        if self.frames.len() == 1 && !self.options.global_return_halts {
            return Err(VmError::IllegalReturnError);
        }

        // Check if we have to remove stack stuff
        if let Some(frame) = self.frames.pop() {
            // We do, so remove everything except for the return value
            let return_value = self.stack.try_pop();
            self.stack.clear_from(frame.stack_offset);
            self.stack.try_push(return_value);
        }

        // Done
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors and taking into account the new StackErrors**
    ///
    /// Sets the value of a global variable.
    /// 
    /// **Arguments**
    ///  * `create_if_not_exists`: If the global doesn't exist, create a new one instead (kinda trivial innit)
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_set_global(&mut self, create_if_not_exists: bool) -> Result<(), VmError> {
        // Get the global's identifier
        let identifier = self.frame().read_constant();
        if let Err(reason) = identifier { return Err(VmError::CallFrameConstError{ what: "a global identifier".to_string(), err: reason }); }
        let identifier = *identifier.unwrap();

        // Get the value to set the global to
        let value = self.stack.pop();
        if let Err(reason) = value { return Err(VmError::StackReadError{ what: "a global variable value".to_string(), err: reason }); }

        // Try to get the string value behind the identifier
        if let Slot::Object(handle) = identifier {
            if let Some(identifier) = self.heap.get(handle) {
                if let Object::String(identifier) = identifier {
                    // TODO: Insert type checking?
                    // Update the value
                    if create_if_not_exists || self.globals.contains_key(identifier) {
                        self.globals.insert(identifier.clone(), value.unwrap());
                    } else {
                        return Err(VmError::UndefinedGlobalError{ identifier: identifier.clone() });
                    }

                    // Done!
                    Ok(())
                } else {
                    Err(VmError::IllegalGlobalIdentifierError{ target: identifier.data_type() })
                }
            } else {
                Err(VmError::DanglingHandleError)
            }
        } else {
            Err(VmError::IllegalGlobalIdentifierError{ target: identifier.into_value(&self.heap).data_type() })
        }
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors and taking into account the new CallFrameErrors**
    ///
    /// Sets the value of a local variable.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_set_local(&mut self) -> Result<(), VmError> {
        // Get the index of the variable to set
        let frame = self.frame();
        let index = frame.read_u8();
        if let Err(reason) = index { return Err(VmError::CallFrame8bitError{ what: "local variable index".to_string(), err: reason }); }
        let index = *index.unwrap() as usize;
        let index = frame.stack_offset + index;

        // Insert the value of the top of the stack there
        self.stack.copy_pop(index);
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning VmErrors**
    ///
    /// Performs a subtraction on the two most recent values on the stack.
    /// 
    /// **Returns**  
    /// Nothing if it was successfull, or a VmError detailling why if it wasn't.
    #[inline]
    pub fn op_substract(&mut self) -> Result<(), VmError> {
        // Get the righthand side from the stack
        let rhs = self.stack.pop();
        if let Err(reason) = rhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let rhs = rhs.unwrap();
        // Get the lefthand side next
        let lhs = self.stack.pop();
        if let Err(reason) = lhs { return Err(VmError::StackReadError{ what: "a numeric value".to_string(), err: reason }); }
        let lhs = lhs.unwrap();

        // Do the division based on what is given to us
        // TODO: Talk about integer VS float division in the documentation.
        match (lhs, rhs) {
            (Slot::Integer(lhs), Slot::Integer(rhs)) => self.stack.push_integer(lhs - rhs),
            (Slot::Integer(lhs), Slot::Real(rhs))    => self.stack.push_real(lhs as f64 - rhs),
            (Slot::Real(lhs), Slot::Real(rhs))       => self.stack.push_real(lhs - rhs),
            (Slot::Real(lhs), Slot::Integer(rhs))    => self.stack.push_real(lhs - rhs as f64),
            _                                        => { return Err(VmError::NotSubtractable{ lhs: lhs.into_value(&self.heap).data_type(), rhs: rhs.into_value(&self.heap).data_type() }) },
        };

        // Done
        Ok(())
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    pub fn op_true(&mut self) {
        self.stack.push(Slot::True);
    }

    ///
    ///
    ///
    #[inline]
    pub fn op_unit(&mut self) {
        self.stack.push(Slot::Unit);
    }
}
