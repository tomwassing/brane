use std::cmp::max;

use crate::frames::{CallFrame, CallFrameError};
use crate::objects::Class;
use crate::stack::{Slot, Stack, StackError};
use crate::{
    builtins,
    builtins::BuiltinFunction,
    builtins::BuiltinError,
    bytecode::{Opcode, FromPrimitive, FunctionMut},
    executor::{VmExecutor, ExecutorError},
    objects::Object,
    objects::{Array, Instance},
    objects::ObjectError,
};
use crate::heap::{Handle, Heap, HeapError};
use fnv::FnvHashMap;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smallvec::SmallVec;
use specifications::common::{FunctionExt, Value};
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
    /// Error for when a package import causes function name conlicts
    DuplicateFunctionImport{ package: String, function: String },
    /// Error for when a package import causes type name conlicts
    DuplicateTypeImport{ package: String, type_name: String },
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

    /// Error for when we want to resolve some object to the heap but we couldn't
    IllegalHandleError{ handle: Handle, err: HeapError },

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
    /// The stack functions could not properly make new slots
    SlotCreateError{ what: String, err: StackError },
    /// Error for when an allocation on the Heap failed
    HeapAllocError{ what: String, err: HeapError },
    /// Error for when we could not freeze something on the Heap
    HeapFreezeError{ what: String, err: HeapError },
    /// Error for when we could not access the Heap
    HeapReadError{ what: String, err: HeapError },
    /// An error occurred while working with objects
    ObjectError{ err: ObjectError },
    /// An error occurred while trying to register the builtins
    BuiltinRegisterError{ err: BuiltinError },
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
            VmError::DuplicateFunctionImport{ package, function } => write!(f, "Package '{}' imports function '{}', but that global variable already exists", package, function),
            VmError::DuplicateTypeImport{ package, type_name }    => write!(f, "Package '{}' imports type '{}', but that global variable already exists", package, type_name),
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

            VmError::IllegalHandleError{ handle, err: HeapError::DanglingHandleError{ handle: _ } } => write!(f, "Encountered dangling handle '{}' on the stack", handle),
            VmError::IllegalHandleError{ handle, err }                                              => write!(f, "Encountered illegal handle '{}' on the stack: {}", handle, err),

            VmError::CallFrameInstrError{ err }         => write!(f, "Could not read next instruction from the callframe: {}", err),
            VmError::CallFrame8bitError{ what, err }    => write!(f, "Could not read {} (8-bit embedded constant) from the callframe: {}", what, err),
            VmError::CallFrame16bitError{ what, err }   => write!(f, "Could not read {} (16-bit embedded constant) from the callframe: {}", what, err),
            VmError::CallFrameConstError{ what, err }   => write!(f, "Could not read {} (a constant) from the callframe: {}", what, err),
            VmError::StackReadError{ what, err }        => write!(f, "Could not read a value ({}) from the stack: {}", what, err),
            VmError::SlotCreateError{ what, err }       => write!(f, "Could not properly create Stack slot for {}: {}", what, err),
            VmError::HeapAllocError{ what, err }        => write!(f, "Could not allocate {} on the heap: {}", what, err),
            VmError::HeapFreezeError{ what, err }       => write!(f, "Could not allocate {} on the heap: {}", what, err),
            VmError::HeapReadError{ what, err }         => write!(f, "Could not read {} from the heap: {}", what, err),
            VmError::ObjectError{ err }                 => write!(f, "An error occurred while working with objects: {}", err),
            VmError::BuiltinRegisterError{ err }        => write!(f, "Could not register builtins: {}", err),
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

    /* TIM */
    /// **Edited: now returns a VmError on errors.**
    ///
    /// Gets the list of globals for this VM, putting values on the heap as needed.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap object to put the global's values on in case they are Objects.
    /// 
    /// **Returns**  
    /// A FnvHashMap containing the globals with their stack slot if we could, or a VmError if we couldn't.
    fn get_globals(
        &self,
        heap: &mut Heap<Object>,
    ) -> Result<FnvHashMap<String, Slot>, VmError> {
        let mut globals = FnvHashMap::default();

        // First process all the the classes.
        for (name, value) in &self.globals {
            if let Value::Class(_) = value {
                let slot = match Slot::from_value(value.clone(), &globals, heap) {
                    Ok(s)       => s,
                    Err(reason) => { return Err(VmError::SlotCreateError{ what: "a global".to_string(), err: reason }); }
                };
                globals.insert(name.clone(), slot);
            }
        }

        // Then the rest of the globals.
        for (name, value) in &self.globals {
            if let Value::Class(_) = value {
                continue;
            } else {
                let slot = match Slot::from_value(value.clone(), &globals, heap) {
                    Ok(s)       => s,
                    Err(reason) => { return Err(VmError::SlotCreateError{ what: "a global".to_string(), err: reason }); }
                };
                globals.insert(name.clone(), slot);
            }
        }

        // Return the list!
        Ok(globals)
    }
    /*******/
}

/// **Edited: now using custom, thread-safe Heap.**
///
/// The VM struct, which represents a VM that can execute either DSL's AST.
pub struct Vm<E>
where
    E: VmExecutor + Clone + Send + Sync,
{
    executor: E,
    frames: SmallVec<[CallFrame; 64]>,
    // frames: Vec<CallFrame>,
    globals: FnvHashMap<String, Slot>,
    heap: Heap<Object>,
    locations: Vec<Handle>,
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

        // Work around the error returned by the new VM
        match Self::new(
            executor,
            frames,
            globals,
            heap,
            locations,
            package_index,
            options,
            stack,
        ) {
            Ok(vm)      => vm,
            Err(reason) => { panic!("Could not create default VM: {}", reason); }
        }
    }
}

impl<E> Vm<E>
where
    E: VmExecutor + Clone + Send + Sync,
{
    /* TIM */
    /// **Edited: Now returns a VmError if the builtin registration can't return properly.**
    ///
    /// Constructor for the Vm class.
    /// 
    /// **Arguments**
    ///  * `executor`: The VmExecutor that will run external jobs for us.
    ///  * `frames`: The list of CallFrames to begin with.
    ///  * `globals`: The map of global defines to begin with.
    ///  * `heap`: The heap to begin with.
    ///  * `locations`: The list of possible locations to run the VmExecutor on.
    ///  * `package_index`: The PackageIndex that determines which packages are available to this Vm.
    ///  * `options`: Options to configure the Vm's behaviour; will also be used in case nested Vms need to be called (to execute nested functions).
    ///  * `stack`: The Stack to begin with.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        executor: E,
        frames: SmallVec<[CallFrame; 64]>,
        globals: FnvHashMap<String, Slot>,
        heap: Heap<Object>,
        locations: Vec<Handle>,
        package_index: PackageIndex,
        options: VmOptions,
        stack: Stack,
    ) -> Result<Self, VmError> {
        let mut globals = globals;
        let mut heap = heap;

        // Register the VM's builtins
        if let Err(reason) = builtins::register(&mut globals, &mut heap) {
            return Err(VmError::BuiltinRegisterError{ err: reason });
        }

        Ok(Self {
            executor,
            frames,
            globals,
            heap,
            locations,
            package_index,
            options,
            stack,
        })
    }

    /* TIM */
    /// **Edited: Now returns a VmError if the globals can't return properly.**
    ///
    /// Tries to create a new Vm with the given resources.
    /// 
    /// **Arguments**
    ///  * `executor`: The Executor to use to run external jobs.
    ///  * `package_index`: The PackageIndex that is used to import external packages.
    ///  * `options`: A list of extra options to initialize the VM with.
    /// 
    /// **Returns**  
    /// A new Vm object on success, or a VmError if we failed to create it.
    pub fn new_with(
        executor: E,
        package_index: Option<PackageIndex>,
        options: Option<VmOptions>,
    ) -> Result<Self, VmError> {
        // Override options, if provided.
        let mut state = VmState::default();
        if let Some(options) = options {
            state.options = options;
        }

        Self::new_with_state(executor, package_index, state)
    }
    /*******/

    /* TIM */
    /// **Edited: Now returns a VmError if the globals can't return properly.**
    ///
    /// Tries to create a new Vm with the given state.
    /// 
    /// **Arguments**
    ///  * `executor`: The Executor to use to run external jobs.
    ///  * `package_index`: The PackageIndex that is used to import external packages.
    ///  * `state`: The VmState to create ourselves with.
    /// 
    /// **Returns**  
    /// A new Vm object on success, or a VmError if we failed to create it.
    pub fn new_with_state(
        executor: E,
        package_index: Option<PackageIndex>,
        state: VmState,
    ) -> Result<Self, VmError> {
        // Initialize the parts of the VM
        let package_index = package_index.unwrap_or_default();
        let mut heap = Heap::default();

        // Create itself
        Ok(Self::new(
            executor,
            Default::default(),
            state.get_globals(&mut heap)?,
            heap,
            Default::default(),
            package_index,
            state.options,
            Stack::default(),
        )?)
    }
    /*******/

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
    /// **Edited: Changed to return VmErrors and handle the new, custom Heap.**
    ///
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

        // Put the main function onto the stack
        let ffunction = match function.freeze(&mut self.heap) {
            Ok(f)       => f,
            Err(reason) => { return Err(VmError::HeapFreezeError{ what: "the main function".to_string(), err: reason }); }
        };
        let function = Object::Function(ffunction);
        let handle = match self.heap.alloc(function) {
            Ok(h)       => h,
            Err(reason) => { return Err(VmError::HeapAllocError{ what: "the main function".to_string(), err: reason }); }
        };

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
    /// **Edited: Changed to return VmErrors and handle the new, custom Heap.**
    /// 
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

        // Put the main function onto the stack
        let ffunction = match function.freeze(&mut self.heap) {
            Ok(f)       => f,
            Err(reason) => { return Err(VmError::HeapFreezeError{ what: "the main function".to_string(), err: reason }); }
        };
        let function = Object::Function(ffunction);
        let handle = match self.heap.alloc(function) {
            Ok(h)       => h,
            Err(reason) => { return Err(VmError::HeapAllocError{ what: "the main function".to_string(), err: reason }); }
        };

        // Run it
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
    /// **Edited: The function signature by adding the Result<(), VmError> return type and internal code to also take into account error handling. Also taking into account the new, custom Heap.**
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
        if let Object::Function(_f) = match self.heap.get(function) {
            Ok(f)       => f,
            Err(reason) => { return Err(VmError::HeapReadError{ what: "a function to call".to_string(), err: reason }); }
        } {
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
            // Get the next instruction, stopping if there aren't any anymore (and erroring on everything else)
            let instruction: Opcode;
            {
                instruction = match self.frame_u8("an instruction") {
                    Ok(instruction) => match Opcode::from_u8(*instruction) {
                        Some(instruction) => instruction,
                        None              => { return Err(VmError::UndefinedOpcodeError{ opcode: *instruction }) }
                    },
                    Err(VmError::CallFrame8bitError{ what: _, err: CallFrameError::IPOutOfBounds{ ip: _, max: _ } }) => { break; }
                    Err(reason)     => { return Err(reason); }
                };
            }

            // Otherwise, switch on the byte we found
            match instruction {
                Opcode::ADD => self.op_add()?,
                Opcode::AND => self.op_and()?,
                Opcode::ARRAY => self.op_array()?,
                Opcode::CALL => self.op_call().await?,
                Opcode::CLASS => self.op_class()?,
                Opcode::CONSTANT => self.op_constant()?,
                Opcode::DEFINE_GLOBAL => self.op_define_global()?,
                Opcode::DIVIDE => self.op_divide()?,
                Opcode::DOT => self.op_dot()?,
                Opcode::EQUAL => self.op_equal()?,
                Opcode::FALSE => self.op_false(),
                Opcode::GET_GLOBAL => self.op_get_global()?,
                Opcode::GET_LOCAL => self.op_get_local()?,
                Opcode::GET_METHOD => self.op_get_method()?,
                Opcode::GET_PROPERTY => self.op_get_property()?,
                Opcode::GREATER => self.op_greater()?,
                Opcode::IMPORT => self.op_import().await?,
                Opcode::INDEX => self.op_index()?,
                Opcode::JUMP => self.op_jump()?,
                Opcode::JUMP_BACK => self.op_jump_back()?,
                Opcode::JUMP_IF_FALSE => self.op_jump_if_false()?,
                Opcode::LESS => self.op_less()?,
                Opcode::LOC => self.op_loc(),
                Opcode::LOC_POP => self.op_loc_pop(),
                Opcode::LOC_PUSH => self.op_loc_push()?,
                Opcode::MULTIPLY => self.op_multiply()?,
                Opcode::NEGATE => self.op_negate()?,
                Opcode::NEW => self.op_new()?,
                Opcode::NOT => self.op_not()?,
                Opcode::OR => self.op_or()?,
                Opcode::PARALLEL => self.op_parallel()?,
                Opcode::POP => self.op_pop()?,
                Opcode::POP_N => self.op_pop_n()?,
                Opcode::RETURN => {
                    self.op_return()?;
                    // Stop if that was the last frame
                    if self.options.global_return_halts && self.frames.is_empty() {
                        break;
                    }
                }
                Opcode::SET_GLOBAL => self.op_set_global(false)?,
                Opcode::SET_LOCAL => self.op_set_local()?,
                Opcode::SUBSTRACT => self.op_substract()?,
                Opcode::TRUE => self.op_true(),
                Opcode::UNIT => self.op_unit(),
            }

            // Try to log
            // No deadlock found...?
            if let Err(reason) = self.executor.debug(format!("Completed instruction {}\n - Stack usage: {} slots\n - Heap usage: {}/{} slots", instruction, self.stack.len(), self.heap.len(), self.heap.capacity())).await {
                warn!("Could not send memory usage statistics to client: {}", reason);
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

    /* TIM */
    // ///
    // ///
    // ///
    // #[inline]
    // fn frame(&mut self) -> &mut CallFrame {
    //     self.frames.last_mut().expect("")
    // }

    /// Given a separate list of frames and heap reference, returns the next byte in the current CallFrame's code.
    /// 
    /// **Arguments**
    ///  * `frames`: The list of CallFrames to read from.
    ///  * `heap`: The Heap to resolve any stack values with.
    ///  * `what`: A string describine what we're getting. Only used in case we fail getting it. Should fill in the phrase: "Could not read ... .".
    /// 
    /// **Returns**  
    /// A reference to the byte's value, or a VmError if we couldn't get it.
    fn frame_u8_sep<'a>(frames: &mut SmallVec<[CallFrame; 64]>, heap: &'a Heap<Object>, what: &str) -> Result<&'a u8, VmError> {
        // Panic if there are no frames
        if frames.len() == 0 { panic!("No CallFrames in VM while running; this should never happen!"); }

        // Get the last element
        let len = frames.len();
        let frame = unsafe { frames.get_unchecked_mut(len - 1) };

        // Now get the u8
        match frame.read_u8(heap) {
            Ok(byte)    => Ok(byte),
            Err(reason) => Err(VmError::CallFrame8bitError{ what: what.to_string(), err: reason }),
        }
    }

    /// Returns the next byte in the current CallFrame's code.
    /// 
    /// **Arguments**
    ///  * `what`: A string describine what we're getting. Only used in case we fail getting it. Should fill in the phrase: "Could not read ... .".
    /// 
    /// **Returns**  
    /// A reference to the byte's value, or a VmError if we couldn't get it.
    #[inline]
    fn frame_u8(&mut self, what: &str) -> Result<&u8, VmError> {
        // Get the frames with Christopher's method to separate references for frames and heap
        let Vm { ref mut frames, ref heap, .. } = self;

        // Use frame_u8_sep for the heavy lifting
        Self::frame_u8_sep(frames, heap, what)
    }

    /// Given a separate list of frames and heap reference, returns the next byte two bytes as a u16 in the current CallFrame's code.
    /// 
    /// **Arguments**
    ///  * `frames`: The list of CallFrames to read from.
    ///  * `heap`: The Heap to resolve any stack values with.
    ///  * `what`: A string describine what we're getting. Only used in case we fail getting it. Should fill in the phrase: "Could not read ... .".
    /// 
    /// **Returns**  
    /// The 16-bit number that was in the code, or a VmError if we couldn't get it.
    fn frame_u16_sep(frames: &mut SmallVec<[CallFrame; 64]>, heap: &Heap<Object>, what: &str) -> Result<u16, VmError> {
        // Panic if there are no frames
        if frames.len() == 0 { panic!("No CallFrames in VM while running; this should never happen!"); }

        // Get the last element
        let len = frames.len();
        let frame = unsafe { frames.get_unchecked_mut(len - 1) };

        // Now get the u16
        match frame.read_u16(heap) {
            Ok(short)   => Ok(short),
            Err(reason) => Err(VmError::CallFrame16bitError{ what: what.to_string(), err: reason }),
        }
    }

    // /// Returns the next byte two bytes as a u16 in the current CallFrame's code.
    // /// 
    // /// **Arguments**
    // ///  * `what`: A string describine what we're getting. Only used in case we fail getting it. Should fill in the phrase: "Could not read ... .".
    // /// 
    // /// **Returns**  
    // /// The 16-bit number that was in the code, or a VmError if we couldn't get it.
    // #[inline]
    // fn frame_u16(&mut self, what: &str) -> Result<u16, VmError> {
    //     // Get the frames with Christopher's method to separate references for frames and heap
    //     let Vm { ref mut frames, ref heap, .. } = self;

    //     // Use frame_u16_sep to do the rest
    //     Self::frame_u16_sep(frames, heap, what)
    // }

    /// Given a separate list of frames and heap reference, returns the next constant value in the current CallFrame's code.
    /// 
    /// **Arguments**
    ///  * `frames`: The list of CallFrames to read from.
    ///  * `heap`: The Heap to resolve any stack values with.
    ///  * `what`: A string describine what we're getting. Only used in case we fail getting it. Should fill in the phrase: "Could not read ... .".
    /// 
    /// **Returns**  
    /// The constant value as a Slot, or a VmError if we couldn't get it.
    fn frame_const_sep<'a>(frames: &mut SmallVec<[CallFrame; 64]>, heap: &'a Heap<Object>, what: &str) -> Result<&'a Slot, VmError> {
        // Panic if there are no frames
        if frames.len() == 0 { panic!("No CallFrames in VM while running; this should never happen!"); }

        // Get the last element
        let len = frames.len();
        let frame = unsafe { frames.get_unchecked_mut(len - 1) };

        // Now get the constant
        match frame.read_constant(heap) {
            Ok(slot)    => Ok(slot),
            Err(reason) => Err(VmError::CallFrameConstError{ what: what.to_string(), err: reason }),
        }
    }

    /// Returns the next constant value in the current CallFrame's code.
    /// 
    /// **Arguments**
    ///  * `what`: A string describine what we're getting. Only used in case we fail getting it. Should fill in the phrase: "Could not read ... .".
    /// 
    /// **Returns**  
    /// The constant value as a Slot, or a VmError if we couldn't get it.
    #[inline]
    fn frame_const(&mut self, what: &str) -> Result<&Slot, VmError> {
        // Get the frames with Christopher's method to separate references for frames and heap
        let Vm { ref mut frames, ref heap, .. } = self;

        // Use frame_const_sep to do the heavy lifting
        Self::frame_const_sep(frames, heap, what)
    }

    /// Returns the stack offset of the current CallFrame's code.
    /// 
    /// **Returns**  
    /// The offset as a usize if we were able to get it, or a VmError if we couldn't.
    fn frame_stack_offset(&mut self) -> Result<usize, VmError> {
        // Get the frames with Christopher's method to separate references for frames and heap
        let Vm { ref mut frames, .. } = self;

        // Panic if there are no frames
        if frames.len() == 0 { panic!("No CallFrames in VM while running; this should never happen!"); }

        // Get the last element
        let len = frames.len();
        let frame = unsafe { frames.get_unchecked_mut(len - 1) };

        // Return the offet
        Ok(frame.stack_offset)
    }
    /*******/

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
            (Slot::Object(lhs_h), Slot::Object(rhs_h))   => {
                // Re-interpret the lefthandside a string
                let lhs = self.heap.get(lhs_h);
                if let Err(reason) = lhs { return Err(VmError::IllegalHandleError{ handle: lhs_h, err: reason }); }
                let slhs = lhs.unwrap();
                // Also do the righthandside
                let rhs = self.heap.get(rhs_h);
                if let Err(reason) = rhs { return Err(VmError::IllegalHandleError{ handle: rhs_h, err: reason }); }
                let srhs = rhs.unwrap();

                // Check if they are indeed strings
                match (slhs, srhs) {
                    (Object::String(lhs), Object::String(rhs)) => {
                        // Concatenate the strings
                        let mut new = lhs.clone();
                        new.push_str(rhs);

                        // Create a new heap object for it
                        let object = match self.heap.alloc(Object::String(new)) {
                            Ok(o)       => o,
                            Err(reason) => { return Err(VmError::HeapAllocError{ what: "a concatenated string".to_string(), err: reason }); }
                        };

                        // Push the object onto the stack
                        self.stack.push_object(object);
                    }
                    _ => { return Err(VmError::NotAddable{ lhs: slhs.data_type(), rhs: srhs.data_type() }); },
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

    /* TIM */
    /// **Edited: working with all kinds of new erros, so returning VmError. Also added new way to read frames and allocate heap.**
    ///
    /// Creates a new Array on the stack.
    /// 
    /// **Returns**  
    /// Nothing if the call was alright, but an Err(VmError) if it couldn't be completed somehow.
    #[inline]
    pub fn op_array(&mut self) -> Result<(), VmError> {
        // Get the number of elements from the callframe
        let n = *self.frame_u8("the number of elements in an Array")?;

        // Construct the list of elements from values on the stack
        let mut elements: Vec<Slot> = Vec::new();
        for i in 0..n {
            // Try to get the stack value
            let val = self.stack.pop();
            if let Err(_) = val { return Err(VmError::ArrayArityError{ got: i, expected: n }); }

            // Add it to the list
            elements.push(val.unwrap());
        }
        elements.reverse();

        // Construct the Array with resolved type
        let mut raw_array = Array::new(elements);
        if let Err(reason) = raw_array.resolve_type(&self.heap) { return Err(VmError::ObjectError{ err: reason }); }
        let array = Object::Array(raw_array);
        
        // Allocate it on the heap
        let handle = match self.heap.alloc(array) {
            Ok(h)       => h,
            Err(reason) => { return Err(VmError::HeapAllocError{ what: "a new array".to_string(), err: reason }); }
        };

        // Push the handle to the Slot and done
        self.stack.push(Slot::Object(handle));
        Ok(())
    }
    /*******/

    /* TIM */
    /// **Edited: now returning errors from buildins (see builtins.rs), local functions and external functions; also edited doc comment**
    ///
    /// Performs an OP_CALL, which call either some builtin, local function or external function that has to be called with the framework.
    /// 
    /// **Returns**  
    /// Nothing if the call was alright, but an Err(VmError) if it couldn't be completed somehow.
    #[inline]
    pub async fn op_call(&mut self) -> Result<(), VmError> {
        debug!("Performing function call...");

        // Get the arity of the callframe (i.e., the number of arguments)
        let arity = *self.frame_u8("a function arity")?;

        // Get the boundries of this frame
        let frame_last = self.stack.len();
        let frame_first = frame_last - (arity + 1) as usize;

        // Get the function pointer
        let function = self.stack.get(frame_first);
        let location = self
            .locations
            .last()
            .map(|l| self.heap.get(*l).unwrap())
            .map(|l| (*l).as_string().cloned().unwrap());

        // Determine how to call
        let value = match function {
            Slot::BuiltIn(code) => {
                debug!("Calling function as builtin '{}'...", code);

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
            Slot::Object(handle) => match self.heap.get(*handle) {
                Ok(Object::Function(_)) => {
                    debug!("Calling function as local function...");

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
                Ok(Object::FunctionExt(f)) => {
                    debug!("Calling function as external function...");

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
                    debug!(" > Handing control to external executor");
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
                Ok(object) => {
                    dbg!(&object);
                    dbg!(&self.stack);
                    panic!("Not a callable object");
                }
                Err(reason) => { return Err(VmError::HeapReadError{ what: "a function to call".to_string(), err: reason }); }
            },
            _ => panic!("Not a callable object"),
        };

        // Remove (built-in or external) function from the stack.
        self.stack.pop().unwrap();

        // Store return value on the stack.
        self.stack.push(match Slot::from_value(value, &self.globals, &mut self.heap) {
            Ok(s)       => s,
            Err(reason) => { return Err(VmError::SlotCreateError{ what: "the result of a function call".to_string(), err: reason }); }
        });

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
        // Push the frame's constant onto the stack
        let class = *self.frame_const("a class")?;
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
        // Push it onto the stack after reading it from the callframe
        let constant = *self.frame_const("a constant")?;
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
        let res = self.frame_const("a property")?;
        let property = res.as_object();
        if let None = property { return Err(VmError::IllegalPropertyError{ target: res.into_value(&self.heap).data_type() }); }
        let property = property.unwrap();

        // Next, try if the object points to an Instance on the heap
        let instance = match self.heap.get(object) {
            Ok(Object::Instance(instance)) => instance,
            Ok(object)  => { return Err(VmError::IllegalDotError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: object, err: reason }); },
        };
        // Now check if the property points to a string on the heap
        let property = match self.heap.get(property) {
            Ok(Object::String(property)) => property,
            Ok(object)  => { return Err(VmError::IllegalPropertyError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: property, err: reason }); },
        };

        // They both do, so finally check if the instance has that property
        let value = instance.properties.get(property);
        if let None = value { return Err(VmError::UndefinedPropertyError{ instance: format!("{}", &instance), property: property.clone() }); }
        let value = *value.unwrap();

        // Finally, push the value of that property on the stack
        self.stack.push(value);

        // Done!
        Ok(())
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
        let identifier = *self.frame_const("a global identifier")?;

        // See if the identifier is a string
        let handle = match identifier {
            Slot::Object(handle) => handle,
            _ => { return Err(VmError::IllegalGlobalIdentifierError{ target: identifier.into_value(&self.heap).data_type() }); }
        };
        // Try to get the identifier as a string from the heap
        let identifier = match self.heap.get(handle) {
            Ok(Object::String(identifier)) => identifier,
            Ok(object)  => { return Err(VmError::IllegalGlobalIdentifierError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: handle, err: reason }); },
        };

        // Get the matching global
        let value = self.globals.get(identifier);
        if let None = value { return Err(VmError::UndefinedGlobalError{ identifier: identifier.clone() }); }
        let value = *value.unwrap();

        // Push its value onto the stack
        self.stack.push(value);

        // Done
        Ok(())
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
        let index = (*self.frame_u8("a local variable offset")?) as usize;
        // Get the stack offset of this CallFrame
        let offset = self.frame_stack_offset()?;

        // Get the matching variable and push it on top of the stack
        self.stack.copy_push(offset + index);
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
        let method = self.frame_const("a method name")?;
        let method_handle = method.as_object();
        if let None = method_handle { return Err(VmError::IllegalPropertyError{ target: method.into_value(&self.heap).data_type() }); }
        let method_handle = method_handle.unwrap();

        // Next, try if the object points to an Instance on the heap
        let instance = match self.heap.get(instance) {
            Ok(Object::Instance(instance)) => instance,
            Ok(object)  => { return Err(VmError::MethodDotError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: instance, err: reason }); },
        };
        // From instance, we move on to try to get the method string
        let method = match self.heap.get(method_handle) {
            Ok(Object::String(method)) => method,
            Ok(object)  => { return Err(VmError::IllegalPropertyError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: method_handle, err: reason }); },
        };
        // Then, we try to obtain the class behind the instance
        let class = match self.heap.get(instance.class) {
            Ok(Object::Class(class)) => class,
            Ok(object)  => { panic!("Instance does not have a Class as baseclass, but a {} ('{}') instead; this should never happen!", object.data_type(), object); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: instance.class, err: reason }); },
        };

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
        Ok(())
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
        let property = self.frame_const("an instance property")?;
        let property_handle = property.as_object();
        if let None = property_handle { return Err(VmError::IllegalPropertyError{ target: property.into_value(&self.heap).data_type() }); }
        let property_handle = property_handle.unwrap();

        // Now check if the object is actually an instance
        let instance = match self.heap.get(instance) {
            Ok(Object::Instance(instance)) => instance,
            Ok(object)  => { return Err(VmError::IllegalDotError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: instance, err: reason }); },
        };
        // Next, check if the property points to a string
        let property = match self.heap.get(property_handle) {
            Ok(Object::String(property)) => property,
            Ok(object)  => { return Err(VmError::IllegalPropertyError{ target: object.data_type() }); }
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: property_handle, err: reason }); }
        };

        // Check if the instance actually has this property
        let value = instance.properties.get(property);
        if let None = value { return Err(VmError::UndefinedPropertyError{ instance: format!("{}", &instance), property: property.clone() }); }
        let value = *value.unwrap();

        // Push the property's value onto the stack
        self.stack.push(value);
        Ok(())
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
        let Vm { ref mut frames, ref heap, .. } = self;
        let p_name = Self::frame_const_sep(frames, heap, "a package identifier")?;
        let p_name_handle = p_name.as_object();
        if let None = p_name_handle { return Err(VmError::IllegalImportError{ target: p_name.into_value(&self.heap).data_type() }); }
        let p_name_handle = p_name_handle.unwrap();

        // Try to get the string behind the handle
        let p_name = match heap.get(p_name_handle) {
            Ok(Object::String(p_name)) => p_name,
            Ok(object)  => { return Err(VmError::IllegalImportError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: p_name_handle, err: reason }); }
        };

        // Try to get the package from the list
        let p_name = p_name.clone();
        let package = self.package_index.get(&p_name, None);
        if let None = package { return Err(VmError::UndefinedImportError{ package: p_name }); }
        let package = package.unwrap();

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
                    kind: package.kind,
                    version: package.version.clone(),
                    parameters: function.parameters.clone(),
                };

                // Write it to the heap
                let handle = match self.heap.alloc(Object::FunctionExt(function)) {
                    Ok(handle)  => handle,
                    Err(reason) => { return Err(VmError::HeapAllocError{ what: "an external function call".to_string(), err: reason }); }
                };
                let object = Slot::Object(handle);

                // Insert the global
                if self.globals.contains_key(f_name) { return Err(VmError::DuplicateFunctionImport{ package: p_name.clone(), function: f_name.clone() }); }
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
                let handle = match self.heap.alloc(Object::Class(class)) {
                    Ok(handle)  => handle,
                    Err(reason) => { return Err(VmError::HeapAllocError{ what: format!("Class '{}'", t_name.clone()), err: reason }); }
                };
                let object = Slot::Object(handle);

                // Insert the global
                if self.globals.contains_key(t_name) { return Err(VmError::DuplicateTypeImport{ package: p_name.clone(), type_name: t_name.clone() }); }
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
        Ok(())
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
        let array_handle = array.unwrap();

        // Try to get the Array behind the stack object
        let array = match self.heap.get(array_handle) {
            Ok(Object::Array(array)) => array,
            Ok(object)  => { return Err(VmError::IllegalIndexError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: array_handle, err: reason }); }
        };

        // Try to get the element from the array
        if let Some(element) = array.elements.get(index as usize) {
            // Put the value on the stack
            self.stack.push(*element);
            Ok(())
        } else {
            Err(VmError::ArrayOutOfBoundsError{ index: index as usize, max: array.elements.len() })
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
        // Read the offset to jump
        let Vm{ ref mut frames, ref heap, .. } = self;
        let offset = Self::frame_u16_sep(frames, heap, "a jump offset")?;
        
        // Update the frame's IP
        let frames_len = frames.len();
        frames[frames_len - 1].ip += offset as usize;
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
        // Read the offset to jump
        let Vm{ ref mut frames, ref heap, .. } = self;
        let offset = Self::frame_u16_sep(frames, heap, "a (backwards) jump offset")?;

        // Update the frame's IP
        let frames_len = frames.len();
        frames[frames_len - 1].ip -= offset as usize;
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
        let truthy = self.stack.peek_boolean();
        if let Err(reason) = truthy { return Err(VmError::StackReadError{ what: "a jump value".to_string(), err: reason }); }

        // Switch on it
        if !truthy.unwrap() {
            // It's a false so jump
            return self.op_jump();
        }

        // Skip the next two bytes detailling the offset
        let frames_len = self.frames.len();
        self.frames[frames_len - 1].ip += 2;
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
        let Vm{ ref mut frames, ref mut heap, .. } = self;
        let properties_n = *Self::frame_u8_sep(frames, heap, "the number of properties")?;

        // Get the class from the stack
        let class = self.stack.pop_object();
        if let Err(reason) = class { return Err(VmError::StackReadError{ what: "a class".to_string(), err: reason }); }
        let class_handle = class.unwrap();

        // Try to resolve the class already
        let class_obj = heap.get(class_handle);
        if let Err(reason) = class_obj { return Err(VmError::IllegalHandleError{ handle: class_handle, err: reason }); }
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
            if let None = key_handle { return Err(VmError::IllegalPropertyError{ target: key.into_value(heap).data_type() }); }
            let key_handle = key_handle.unwrap();
            // Get the property value
            let val = self.stack.pop();
            if let Err(reason) = val { return Err(VmError::StackReadError{ what: "a property value".to_string(), err: reason }); }

            // Try if the key is a string
            let key = match heap.get(key_handle) {
                Ok(Object::String(key)) => key,
                Ok(object)  => { return Err(VmError::IllegalPropertyError{ target: object.data_type() }); },
                Err(reason) => { return Err(VmError::IllegalHandleError{ handle: key_handle, err: reason }); }
            };

            // Insert the key/value pair
            properties.insert(key.clone(), val.unwrap());
        }

        // Get the class behind the handle
        if let Object::Class(c) = class_obj {
            // Get the name of the class
            let c_name = c.name.clone();

            // Create a new instance from it on the heap
            let instance = Instance::new(class_handle, properties);
            match heap.alloc(Object::Instance(instance)) {
                Ok(instance) => {
                    // Put the instance on the stack
                    self.stack.push_object(instance);
                    Ok(())
                },
                Err(reason)  => Err(VmError::HeapAllocError{ what: format!("a new Instance of Class '{}'", c_name), err: reason }),
            }
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
        let x = *self.frame_u8("the number of stack items to pop")? as usize;

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
        let identifier = *self.frame_const("a global identifier")?;

        // Get the value to set the global to
        let value = self.stack.pop();
        if let Err(reason) = value { return Err(VmError::StackReadError{ what: "a global variable value".to_string(), err: reason }); }

        // Try to get the string value behind the identifier
        let handle = match identifier {
            Slot::Object(handle) => handle,
            _ => { return Err(VmError::IllegalGlobalIdentifierError{ target: identifier.into_value(&self.heap).data_type() }); }
        };
        let identifier = match self.heap.get(handle) {
            Ok(Object::String(identifier)) => identifier,
            Ok(object)  => { return Err(VmError::IllegalGlobalIdentifierError{ target: object.data_type() }); },
            Err(reason) => { return Err(VmError::IllegalHandleError{ handle: handle, err: reason }); }
        };

        // TODO: Insert type checking?
        // Update the value
        if create_if_not_exists || self.globals.contains_key(identifier) {
            self.globals.insert(identifier.clone(), value.unwrap());
        } else {
            return Err(VmError::UndefinedGlobalError{ identifier: identifier.clone() });
        }

        // Done!
        Ok(())
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
        let Vm{ ref mut frames, ref heap, .. } = self;
        let index = *Self::frame_u8_sep(frames, heap, "a local variable index")? as usize;

        // Get the frame offset
        let offset = self.frame_stack_offset()?;

        // Insert the value of the top of the stack there
        self.stack.copy_pop(offset + index);
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
