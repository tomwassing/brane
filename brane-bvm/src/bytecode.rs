use crate::objects::{self, Class, Object};
use crate::stack::Slot;
use crate::Function;
use crate::heap::{Heap, HeapError};

pub use num_traits::{FromPrimitive, ToPrimitive};
use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use fnv::FnvHashMap;
use specifications::common::{Bytecode, SpecClass, SpecFunction, Value};

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult, Write};


/***** ERRORS *****/
/// Defines errors in the bytecode.
#[derive(Debug, PartialEq)]
pub enum BytecodeError {
    /// Encountered an unknown instruction
    UnknownInstruction{ instruction: u8 },
    /// Could not write to the disassemble string during disassembly
    DissasembleWriteError{ err: std::fmt::Error },
    /// Could not successfully allocate something on the heap
    HeapAllocateError{ err: HeapError },
}

impl From<std::fmt::Error> for BytecodeError {
    #[inline]
    fn from(value: std::fmt::Error) -> Self {
        BytecodeError::DissasembleWriteError{ err: value }
    }
}

impl From<HeapError> for BytecodeError {
    #[inline]
    fn from(value: HeapError) -> Self {
        BytecodeError::HeapAllocateError{ err: value }
    }
}

impl Display for BytecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            BytecodeError::UnknownInstruction{ instruction } => write!(f, "Encountered unknown instruction opcode '{}'", instruction),
            BytecodeError::DissasembleWriteError{ err }      => write!(f, "Could not write disassembly to string: {}", err),
            BytecodeError::HeapAllocateError{ err }          => write!(f, "Could not allocate new object on the Heap: {}", err),
        }
    }
}

impl Error for BytecodeError {}





/***** ENUMS *****/
/// Defines the opcodes in the Brane VM
#[repr(u8)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, FromPrimitive, ToPrimitive)]
pub enum Opcode {
    /// Performs an arithmetic add on the top two items on the stack.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (either an int, float or string) of the calculation on the top of the stack.
    ///  * The lefthandside (either an int, float or string) of the calculation as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the calculation on top of the stack, carrying the same type as the input arguments.
    ADD = 0x01,

    /// Performs a logical conjunction on the top two items on the stack.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (a boolean) of the comparison on the top of the stack.
    ///  * The lefthandside (a boolean) of the comparison as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the comparison on top of the stack, as a boolean.
    AND = 0x02,

    /// Creates an Array on the heap from the last N elements on the stack.
    /// 
    /// **Code arguments**
    ///  * A single byte detailling the size of the Array.
    /// 
    /// **Stack arguments**
    ///  * At least N values on top of the stack that are of the same type, where N is the size of the array defined in the callframe.
    /// 
    /// **Results**
    ///  * A new Array handle on top of the stack, and the actual Array allocated on the heap.
    ARRAY = 0x03,

    /// Performs a function call, possibly external if the function on top of the stack is.
    /// 
    /// **Code arguments**
    ///  * `A single byte defining the arity (number of arguments) for this function.
    /// 
    /// **Stack arguments**
    ///  * The parameters as top N values on the stack, where N is the arity of this function defined in the callframe.
    ///  * Below that, (a handle to) the function definition as a Builtin, Function or FunctionExt.
    /// 
    /// **Results**
    ///  * The result of the call on top of the stack.
    CALL = 0x04,

    /// Creates a new class type on the stack
    /// 
    /// **Code arguments**
    ///  * The type definition as constant in the callframe (i.e., a byte in the callframe itself referencing to some data in the callframe's constant area).
    /// 
    /// **Results**
    ///  * A handle to the new Class object describing the custom type.
    CLASS = 0x05,

    /// Moves a constant from the callframe to the stack
    /// 
    /// **Code arguments**
    ///  * A constant in the callframe (i.e., a byte in the callframe itself referencing to some data in the callframe's constant area)
    /// 
    /// **Results**
    ///  * The new value on top of the stack.
    CONSTANT = 0x06,

    /// Creates a new global with its identifier in the callframe and its new value on the top of the stack.
    /// 
    /// **Code arguments**
    ///  * The identifier of the global stored as a string in the callframe constant area (so it's actually a byte pointing to it)
    /// 
    /// **Stack arguments**
    ///  * The top value of the stack that will be used to define the type and value of the new global.
    /// 
    /// **Results**
    ///  * Nothing on the stack, but instead a new entry in the global table.
    DEFINE_GLOBAL = 0x07,

    /// Performs an division add on the top two items on the stack.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (either an int or float) of the calculation on the top of the stack.
    ///  * The lefthandside (either an int or float) of the calculation as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the calculation on top of the stack, carrying the same type as the input arguments.
    DIVIDE = 0x08,

    /// Access the value of a property from an instance.
    /// 
    /// **Code arguments**
    ///  * The identifier of the property stored as a string in the callframe constant area (so it's actually a byte pointing to it)
    /// 
    /// **Stack arguments**
    ///  * The instance that we mean to access on top of the stack. This also defines what the type is of the object we're accessing.
    /// 
    /// **Results**
    ///  * The value of the property op top of the stack.
    DOT = 0x09,

    /// Checks if the top two values on the stack are equal to each other.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (anything) of the comparison on the top of the stack.
    ///  * The lefthandside (anything) of the comparison as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the comparison on top of the stack, as a boolean.
    EQUAL = 0x0A,

    /// Pushes a simple False boolean onto the stack.
    /// 
    /// **Results**
    ///  * A new boolean that is False on top of the stack.
    FALSE = 0x0B,

    /// Returns the value of the given global variable.
    /// 
    /// **Code arguments**
    ///  * The identifier of the global stored as a string in the callframe constant area (so it's actually a byte pointing to it)
    /// 
    /// **Results**
    ///  * The value of the global identifier pushed on top of the stack.
    GET_GLOBAL = 0x0C,

    /// Returns the value of the given local variable.
    /// 
    /// **Code arguments**
    ///  * The offset of the local variable in the current stack, as a single byte.
    /// 
    /// **Results**
    ///  * The value of the local variable in the stack. Note that the original value is left untouched, and instead a copy is returned.
    GET_LOCAL = 0x0D,

    /// Returns a method belonging to an instance.
    /// 
    /// **Code arguments**
    ///  * The identifier of the method stored as a string in the callframe constant area (so it's actually a byte pointing to it).
    /// 
    /// **Stack arguments**
    ///  * The instance that we mean to access on top of the stack. This also defines what the type is of the object we're accessing.
    /// 
    /// **Results**
    ///  * The method (as a Function or FunctionExt) on top of the stack so that it can be called.
    GET_METHOD = 0x26,

    /// Access the value of a property from an instance.
    /// 
    /// Seems to do exactly the same as OP_DOT??
    /// 
    /// **Code arguments**
    ///  * The identifier of the property stored as a string in the callframe constant area (so it's actually a byte pointing to it)
    /// 
    /// **Stack arguments**
    ///  * The instance that we mean to access on top of the stack. This also defines what the type is of the object we're accessing.
    /// 
    /// **Results**
    ///  * The value of the property op top of the stack.
    GET_PROPERTY  = 0x27,

    /// Checks if the top two values on the stack if the lefthandside is larger than the righthandside.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (an integer or a float) of the comparison on the top of the stack.
    ///  * The lefthandside (an integer or a float) of the comparison as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the comparison on top of the stack, as a boolean.
    GREATER = 0x0E,

    /// Imports the functions and types of a package into global memory.
    /// 
    /// **Code arguments**
    ///  * The identifier of the package stored as a string in the callframe constant area (so it's actually a byte pointing to it).
    /// 
    /// **Results**
    ///  * Each of the functions the package exports as a global variable (so that's a FunctionExt).
    ///  * Each of the types the package exports as a global variable (so that's a Class).
    IMPORT = 0x0F,

    /// Indexes a given array and returns the value of the referred element.
    /// 
    /// **Stack arguments**
    ///  * The index of the array on the top of the stack, as an integer.
    ///  * A handle to the array itself, just below that.
    /// 
    /// **Results**
    ///  * The value of the indexed element of the array on top of the stack.
    INDEX = 0x10,

    /// Moves the instruction pointer in the current frame _forward_.
    /// 
    /// **Code arguments**
    ///  * The offset to jump, as an unsigned, 16-bit integer (so that's two bytes).
    /// 
    /// **Results**
    ///  * Nothing on the stack, just moves the callframe pointer.
    JUMP = 0x11,

    /// Moves the instruction pointer in the current frame _backward_.
    /// 
    /// **Code arguments**
    ///  * The offset to jump, as an unsigned, 16-bit integer (so that's two bytes).
    /// 
    /// **Results**
    ///  * Nothing on the stack, just moves the callframe pointer.
    JUMP_BACK = 0x12,

    /// Moves the instruction pointer in the current frame _forward_ if the top value on the stack is false.
    /// 
    /// **Code arguments**
    ///  * The offset to jump, as an unsigned, 16-bit integer (so that's two bytes).
    /// 
    /// **Stack arguments**
    ///  * The boolean to jump conditionally on on top of the stack. Note that this boolean isn't popped of the stack, but left on there instead.
    /// 
    /// **Results**
    ///  * Nothing on the stack, just moves the callframe pointer.
    JUMP_IF_FALSE = 0x13,

    /// Checks if the top two values on the stack if the lefthandside is smaller than the righthandside.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (an integer or a float) of the comparison on the top of the stack.
    ///  * The lefthandside (an integer or a float) of the comparison as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the comparison on top of the stack, as a boolean.
    LESS = 0x14,

    /// Pops the top location off the location stack, returning it on the stack.
    /// 
    /// **Location arguments**
    ///  * The location to pop off the location stack.
    /// 
    /// **Results**
    ///  * The top location of the location stack as a Value, popped onto the stack. Note that, if there is no location on the location stack, this returns a Unit instead of crashing.
    LOC = 0x25,

    /// Pops the top location off the location stack, not returning anything.
    /// 
    /// **Location arguments**
    ///  * The location to pop off the location stack.
    LOC_POP = 0x15,

    /// Pushes the top value of the stack to the location stack.
    /// 
    /// **Stack arguments**
    ///  * An object that represents some location to push to the location stack.
    /// 
    /// **Results**
    ///  * A new location on the location stack.
    LOC_PUSH = 0x16,

    /// Performs an arithmetic multiplication on the top two items on the stack.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (either an int or float) of the calculation on the top of the stack.
    ///  * The lefthandside (either an int or float) of the calculation as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the calculation on top of the stack, carrying the same type as the input arguments.
    MULTIPLY = 0x17,

    /// Performs an arithmetic negation on the top item on the stack.
    /// 
    /// **Stack arguments**
    ///  * The item (either an int or float) to negate on top of the stack.
    /// 
    /// **Results**
    ///  * The negated version of the top item in its place.
    NEGATE = 0x18,

    /// Instantiates a given Object.
    /// 
    /// **Code arguments**
    ///  * The 'arity' of the Object to instantiate, i.e., how many properties it has (as a byte).
    /// 
    /// **Stack arguments**
    ///  * The Class/type definition on top of the stack.
    ///  * At least N properties, where N is the number of properties read from the code. Note that the type of these properties depend on the class definition.
    /// 
    /// **Results**
    ///  * A handle to a newly instantiated Instance object, who's type is that of the class definition that was on top.
    NEW = 0x19,

    /// Performs a logical not-operation on the top item on the stack.
    /// 
    /// **Stack arguments**
    ///  * The item (as a boolean) to flip on top of the stack.
    /// 
    /// **Results**
    ///  * The flipped version of the top boolean in its place.
    NOT = 0x1A,

    /// Performs a logical disjunction on the top two items on the stack.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (a boolean) of the comparison on the top of the stack.
    ///  * The lefthandside (a boolean) of the comparison as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the comparison on top of the stack, as a boolean.
    OR = 0x1B,

    /// TODO: Fix implementation + write documentation
    PARALLEL = 0x1C,

    /// Pops the top value off the stack.
    /// 
    /// **Stack arguments**
    ///  * The element to quite ungloriously discard on top of the stack.
    POP = 0x1D,

    /// Pops the top N values off the stack.
    /// 
    /// **Code arguments**
    ///  * The number of values to pop, as a byte.
    /// 
    /// **Stack arguments**
    ///  * The top N elements to quite ungloriously discard, where N is the number of elements to discard as given by the code.
    POP_N = 0x1E,

    /// Pops the current frame from the callstack, returning to the calling function.
    /// 
    /// **Stack arguments**
    ///  * If there is a value left for this frame's stack, tries to return it.
    /// 
    /// **Results**
    ///  * Removes everything from the call- / normal-stack belonging to the current call. If the VM option 'global_return_halts' is set and this was the last frame (the global frame), then the VM stops execution.
    ///  * If there was a value, the return value is now on top of the stack.
    RETURN = 0x1F,

    /// Sets the value for a global variable.
    /// 
    /// **Code arguments**
    ///  * The identifier of the variable stored as a string in the callframe constant area (so it's actually a byte pointing to it)
    /// 
    /// **Stack arguments**
    ///  * The new value that the global should be set to on top of the stack. Note that this quite happily overwrites any type the global already has.
    /// 
    /// **Results**
    ///  * Nothing on the stack, but a new value for the given global in the global table.
    SET_GLOBAL = 0x20,

    /// Sets the value for a local variable.
    /// 
    /// **Code arguments**
    ///  * The offset of the local variable in the current stack, as a single byte.
    /// 
    /// **Stack arguments**
    ///  * The new value that the local should be set to on top of the stack. Note that this quite happily overwrites any type the local already has.
    /// 
    /// **Results**
    ///  * Nothing on top of the stack, but a new value for the given local somewhere down in the stack.
    SET_LOCAL = 0x21,

    /// Performs an arithmetic subtraction on the top two items on the stack.
    /// 
    /// **Stack arguments**
    ///  * The righthandside (either an int or float) of the calculation on the top of the stack.
    ///  * The lefthandside (either an int or float) of the calculation as second on the stack.
    /// 
    /// **Results**
    ///  * The result of the calculation on top of the stack, carrying the same type as the input arguments.
    SUBSTRACT = 0x22,

    /// Pushes a simple True boolean onto the stack.
    /// 
    /// **Results**
    ///  * A new boolean that is True on top of the stack.
    TRUE = 0x23,

    /// Pushes a simple Unit (void value) onto the stack.
    /// 
    /// **Results**
    ///  * A new Unit on top of the stack.
    UNIT = 0x24,
}

impl Into<u8> for Opcode {
    #[inline]
    fn into(self) -> u8 {
        self.to_u8().unwrap()
    }
}

impl Display for Opcode {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "OP_{:?}", self)
    }
}




/***** HELPER FUNCTIONS *****/
/// Prints out a jump instruction neatly.
/// 
/// **Arguments**
///  * `name`: The name of the instruction.
///  * `sign`: The sign of the jump to perform (1 = forwards, -1 = backwards).
///  * `chunk`: The bytecode Chunk to get the jump offset from.
///  * `offset`: The offset into the bytecode where instruction opcode is located.
///  * `result`: The String to write to.
fn jump_instruction(
    name: &str,
    sign: i16,
    chunk: &Chunk,
    offset: usize,
    result: &mut String,
) {
    let jump1 = chunk.code[offset + 1] as u16;
    let jump2 = chunk.code[offset + 2] as u16;

    let jump = (jump1 << 8) | jump2;
    writeln!(
        result,
        "{:<16} {:4} -> {}",
        name,
        offset,
        offset as i32 + 3 + (sign * jump as i16) as i32
    )
    .unwrap();
}

/// Prints out a constant instruction neatly.
/// 
/// **Arguments**
///  * `name`: The name of the instruction.
///  * `chunk`: The bytecode Chunk to get the constant value from.
///  * `offset`: The offset into the bytecode where instruction opcode is located.
///  * `result`: The String to write to.
fn constant_instruction(
    name: &str,
    chunk: &Chunk,
    offset: usize,
    result: &mut String,
) {
    let constant = chunk.code[offset + 1];
    write!(result, "{:<16} {:4} | ", name, constant).unwrap();

    if let Some(value) = chunk.constants.get(constant as usize) {
        writeln!(result, "{:?}", value).unwrap();
    }
}

/// Prints out a stack instruction neatly.
/// 
/// **Arguments**
///  * `name`: The name of the instruction.
///  * `chunk`: The bytecode Chunk to get the slot from.
///  * `offset`: The offset into the bytecode where instruction opcode is located.
///  * `result`: The String to write to.
fn byte_instruction(
    name: &str,
    chunk: &Chunk,
    offset: usize,
    result: &mut String,
) {
    let slot = chunk.code[offset + 1];
    writeln!(result, "{:<16} {:4} | ", name, slot).unwrap();
}





/***** LIBRARY STRUCTS *****/
/// A muteable, workeable version of the Object Class.
#[derive(Clone)]
pub struct ClassMut {
    /// The typename of this class
    pub name       : String,
    /// The list of properties for this Class.
    pub properties : HashMap<String, String>,
    /// The list of methods for this Class.
    pub methods    : HashMap<String, FunctionMut>,
}

impl ClassMut {
    /// Constructor for the ClassMut.
    /// 
    /// **Arguments**
    ///  * `name`: The typename of the class.
    ///  * `properties`: The properties for this Class (usually an empty dict at this point).
    ///  * `methods`: The list of methods that this Class implements.
    #[inline]
    pub fn new(
        name: String,
        properties: HashMap<String, String>,
        methods: HashMap<String, FunctionMut>,
    ) -> Self {
        Self {
            name,
            properties,
            methods,
        }
    }



    /// **Edited to work with custom Heap and returning BytecodeErrors.**
    ///
    /// Freezes the class onto the heap, effectively freezing all its functions on it with the class definition on top.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap to freeze the class on.
    /// 
    /// **Returns**  
    /// The heap-frozen Class if everything went alright, or a BytecodeError otherwise.
    pub fn freeze(
        self,
        heap: &mut Heap<Object>,
    ) -> Result<Class, BytecodeError> {
        // Freeze each method
        let mut methods: FnvHashMap<String, Slot> = Default::default();
        for (k, v) in self.methods {
            // Freeze it and put it on the stack
            let function = v.freeze(heap)?;
            let handle = heap.alloc(Object::Function(function))?;
            let slot = Slot::Object(handle);
            
            // Return the object as new value
            methods.insert(k, slot);
        };

        // Return the Class with the frozen methods
        Ok(Class {
            name: self.name,
            methods,
        })
    }
}

impl From<SpecClass> for ClassMut {
    fn from(f: SpecClass) -> Self {
        let methods = f.methods.iter().map(|(k, v)| (k.clone(), v.clone().into())).collect();
        Self::new(f.name, f.properties, methods)
    }
}

impl From<ClassMut> for SpecClass {
    fn from(f: ClassMut) -> Self {
        let methods = f.methods.iter().map(|(k, v)| (k.clone(), v.clone().into())).collect();
        Self::new(f.name, f.properties, methods)
    }
}



/// A muteable, workeable version of the Object Function.
#[derive(Clone)]
pub struct FunctionMut {
    /// The function's arity (i.e., number of arguments).
    pub arity : u8,
    /// The Chunk that defines this function's bytecode.
    pub chunk : ChunkMut,
    /// The name of this function.
    pub name  : String,
}

impl FunctionMut {
    /// Constructor for the FunctionMut that initializes it as the Main function (no inputs).
    /// 
    /// **Arguments**
    ///  * `chunk`: The Chunk that defines the bytecode for the main function.
    #[inline]
    pub fn main(chunk: ChunkMut) -> Self {
        Self {
            arity: 0,
            chunk,
            name: String::from("main"),
        }
    }

    /// Constructor for the FunctionMut.  
    /// To initialize the main function, it's preferred to use FunctionMut::main() instead.
    /// 
    /// **Arguments**
    ///  * `name`: The name of the function. Cannot be 'main'.
    ///  * `arity`: The arity (=number of arguments) of this function. Cannot be 0.
    ///  * `chunk`: The Chunk that defines the bytecode for this function.
    #[inline]
    pub fn new(
        name: String,
        arity: u8,
        chunk: ChunkMut,
    ) -> Self {
        Self { arity, chunk, name }
    }



    /// **Edited: Now returning a BytecodeError**
    ///
    /// Freezes the function onto the heap.  
    /// Here, it means that the bytecode will be frozen onto the heap.
    /// 
    /// **Returns**  
    /// A frozen Function if we could freeze it, or a BytecodeError otherwise.
    #[inline]
    pub fn freeze(
        self,
        heap: &mut Heap<Object>,
    ) -> Result<objects::Function, BytecodeError> {
        Ok(Function::new(self.name, self.arity, self.chunk.freeze(heap)?))
    }
}

impl From<SpecFunction> for FunctionMut {
    fn from(f: SpecFunction) -> Self {
        let chunk = ChunkMut::new(f.bytecode.code[..].into(), f.bytecode.constants);
        Self::new(f.name, f.arity, chunk)
    }
}

impl From<FunctionMut> for SpecFunction {
    fn from(f: FunctionMut) -> Self {
        SpecFunction {
            arity: f.arity,
            name: f.name,
            bytecode: Bytecode {
                code: f.chunk.code[..].to_vec(),
                constants: f.chunk.constants,
            },
        }
    }
}



/// A list of bytecode.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The bytecode, in, well, Bytes.
    pub code      : Bytes,
    /// A list of extra constants that are part of this Chunk.
    pub constants : Vec<Slot>,
}

impl Chunk {
    /// **Edited: now using Opcodes instead of numbers and returning BytecodeErrors.**
    /// 
    /// Disassembles the Chunk into a String showing human-readable assembly from the bytecode.
    /// 
    /// **Returns**  
    /// The human-readable String on success, or else a BytecodeError upon failure.
    pub fn disassemble(&self) -> Result<String, BytecodeError> {
        let mut result = String::new();
        let mut skip = 0;

        // Iterate through all the bytes
        for (offset, instruction) in self.code.iter().enumerate() {
            // If told to skip some bytes, do so
            if skip > 0 {
                skip -= 1;
                continue;
            }

            // Try to convert the instruction to an offset
            let instruction = match Opcode::from_u8(*instruction) {
                Some(instruction) => instruction,
                None              => { return Err(BytecodeError::UnknownInstruction{ instruction: *instruction }); }
            };

            // Write the string representation of each opcode
            write!(result, "{:04} ", offset)?;
            match instruction {
                // Opcodes we can immediately print without hassle
                Opcode::ADD       |
                Opcode::AND       |
                Opcode::DIVIDE    |
                Opcode::EQUAL     |
                Opcode::FALSE     |
                Opcode::GREATER   |
                Opcode::INDEX     |
                Opcode::LESS      |
                Opcode::LOC       |
                Opcode::LOC_POP   |
                Opcode::LOC_PUSH  |
                Opcode::MULTIPLY  |
                Opcode::NEGATE    |
                Opcode::NOT       |
                Opcode::OR        |
                Opcode::POP       |
                Opcode::RETURN    |
                Opcode::SUBSTRACT |
                Opcode::TRUE      |
                Opcode::UNIT      => {
                    writeln!(result, "{}", &format!("{}", instruction))?;
                }

                // Opcodes which we write with a constant argument
                Opcode::CLASS         |
                Opcode::CONSTANT      |
                Opcode::DEFINE_GLOBAL |
                Opcode::DOT           |
                Opcode::GET_GLOBAL    |
                Opcode::GET_METHOD    |
                Opcode::GET_PROPERTY  |
                Opcode::IMPORT        => {
                    constant_instruction(&format!("{}", instruction), self, offset, &mut result);
                    skip = 1;
                }

                // Opcodes which we write as an instruction with some extra byte argument
                Opcode::ARRAY      |
                Opcode::CALL       |
                Opcode::GET_LOCAL  |
                Opcode::NEW        |
                Opcode::PARALLEL   |
                Opcode::POP_N      |
                Opcode::SET_GLOBAL |
                Opcode::SET_LOCAL  => {
                    byte_instruction(&format!("{}", instruction), self, offset, &mut result);
                    skip = 1;
                }

                // Opcodes which we write as an instruction plus a jump offset
                Opcode::JUMP => {
                    jump_instruction(&format!("{}", instruction), 1, self, offset, &mut result);
                    skip = 2;
                }
                Opcode::JUMP_BACK => {
                    jump_instruction(&format!("{}", instruction), -1, self, offset, &mut result);
                    skip = 2;
                }
                Opcode::JUMP_IF_FALSE => {
                    jump_instruction(&format!("{}", instruction), 1, self, offset, &mut result);
                    skip = 2;
                }
            }
        }

        Ok(result)
    }



    /// Unfreezes the Chunk into a ChunkMut, consuming it.  
    /// Here, it means the constants are translated into values and the Bytes are made muteable.
    /// 
    /// **Returns**  
    /// The new ChunkMut that is this Chunk but unfrozen.
    pub fn unfreeze(self) -> ChunkMut {
        // Translate the constant Slots into constant Values.
        let constants = self.constants.into_iter().map(|s| s.into_value()).collect();
        // Return them in a ChunkMut
        ChunkMut::new(BytesMut::from(&self.code[..]), constants)
    }
}



/// A muteable, workeable version of the Chunk.
#[derive(Clone, Debug)]
pub struct ChunkMut {
    /// The bytecode, in, well, bytes (but muteable).
    pub code      : BytesMut,
    /// A list of extra constants that are part of this ChunkMut.
    pub constants : Vec<Value>,
}

impl Default for ChunkMut {
    /// Default constructor for the ChunkMut.
    #[inline]
    fn default() -> Self {
        Self {
            code: BytesMut::default(),
            constants: Vec::default(),
        }
    }
}

impl ChunkMut {
    /// Constructor for the ChunkMut.
    /// 
    /// **Arguments**
    ///  * `code`: The (muteable) bytecode to wrap this chunk around.
    ///  * `constants`: The list of extra constants that will be part of this ChunkMut.
    #[inline]
    pub fn new(
        code: BytesMut,
        constants: Vec<Value>,
    ) -> Self {
        ChunkMut { code, constants }
    }



    /// Writes a new byte to this chunk.
    /// 
    /// **Arguments**
    ///  * `byte`: The byte(-like) to add to the chunk.
    #[inline]
    pub fn write<B: Into<u8>>(&mut self, byte: B) {
        self.code.put_u8(byte.into());
    }

    /// Writes a new set of two bytes to this chunk.  
    /// Convenience function for calling write() twice.
    /// 
    /// **Arguments**
    ///  * `byte1`: The first byte(-like) to add to the chunk.
    ///  * `byte2`: The second byte(-like) to add to the chunk.
    pub fn write_pair<B1: Into<u8>, B2: Into<u8>>(
        &mut self,
        byte1: B1,
        byte2: B2,
    ) {
        self.write(byte1);
        self.write(byte2);
    }

    /// Writes a vector of bytes to this chunk.
    /// 
    /// **Arguments**
    ///  * `bytes`: Vector of bytes to add to the of this chunk.
    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.code.extend(bytes);
    }



    /// Writes a new constant to this chunk.
    /// 
    /// **Arguments** 
    ///  * `value`: The Value to write as a constant.
    /// 
    /// **Returns**  
    /// The constant index to this constant, which can be used in the bytecode to reference to it.
    pub fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);
        (self.constants.len() as u8) - 1
    }



    /// **Edited to work with custom Heap and returning BytecodeErrors.**
    ///
    /// Freezes a function body / chunk of code on the heap.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap to freeze the class on.
    /// 
    /// **Returns**  
    /// The heap-frozen Chunk if everything went alright, or a BytecodeError otherwise.
    pub fn freeze(
        self,
        heap: &mut Heap<Object>,
    ) -> Result<Chunk, BytecodeError> {
        // Freeze the constants manually
        let mut constants: Vec<Slot> = Vec::with_capacity(self.constants.len());
        for c in self.constants {
            constants.push(match c {
                Value::Boolean(b) => match b {
                    true => Slot::True,
                    false => Slot::False,
                },
                Value::Integer(i) => Slot::Integer(i),
                Value::Real(r) => Slot::Real(r),
                Value::Function(f) => {
                    // Freeze the function first
                    let f = FunctionMut::from(f);
                    let function = Object::Function(f.freeze(heap)?);
                    let handle = heap.alloc(function)?;

                    // Return the handle
                    Slot::Object(handle)
                }
                Value::Unicode(s) => {
                    // Freeze the string first
                    let string = Object::String(s);
                    let handle = heap.alloc(string)?;

                    // Return the handle
                    Slot::Object(handle)
                }
                Value::Class(c) => {
                    // Freeze the methods first
                    let mut methods = FnvHashMap::default();

                    for (name, method) in c.methods.clone().into_iter() {
                        // Freeze the function
                        let method_mut: FunctionMut = method.into();
                        let method = Object::Function(method_mut.freeze(heap)?);

                        // Return the handle for the class itself
                        let handle = heap.alloc(method)?;
                        methods.insert(name, Slot::Object(handle));
                    }

                    // Construct the class
                    let class = Class { name: c.name, methods };

                    // Put the class on the heap as well
                    let class = Object::Class(class);
                    let handle = heap.alloc(class)?;

                    // Return the class
                    Slot::Object(handle)
                }
                a => {
                    // Unsupported constant; quit ungracefully
                    panic!("Encountered unsupported constant of type '{}' ('{}'); this should never happen!", a.data_type(), a);
                }
            });
        }

        Ok(Chunk {
            code: self.code.freeze(),
            constants,
        })
    }
}
