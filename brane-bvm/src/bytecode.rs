use crate::objects::{self, Class, Object};
use crate::stack::Slot;
use crate::Function;
use anyhow::Result;
use crate::heap::{Heap, HeapError};
use bytes::{BufMut, Bytes, BytesMut};
use fnv::FnvHashMap;
use specifications::common::{Bytecode, SpecClass, SpecFunction, Value};
use std::collections::HashMap;
use std::fmt::Write;


pub mod opcodes {
    pub const OP_ADD: u8 = 0x01;
    pub const OP_AND: u8 = 0x02;
    pub const OP_ARRAY: u8 = 0x03;
    pub const OP_CALL: u8 = 0x04;
    pub const OP_CLASS: u8 = 0x05;
    pub const OP_CONSTANT: u8 = 0x06;
    pub const OP_DEFINE_GLOBAL: u8 = 0x07;
    pub const OP_DIVIDE: u8 = 0x08;
    pub const OP_DOT: u8 = 0x09;
    pub const OP_EQUAL: u8 = 0x0A;
    pub const OP_FALSE: u8 = 0x0B;
    pub const OP_GET_GLOBAL: u8 = 0x0C;
    pub const OP_GET_LOCAL: u8 = 0x0D;
    pub const OP_GET_METHOD: u8 = 0x26;
    pub const OP_GET_PROPERTY: u8 = 0x27;
    pub const OP_GREATER: u8 = 0x0E;
    pub const OP_IMPORT: u8 = 0x0F;
    pub const OP_INDEX: u8 = 0x10;
    pub const OP_JUMP: u8 = 0x11;
    pub const OP_JUMP_BACK: u8 = 0x12;
    pub const OP_JUMP_IF_FALSE: u8 = 0x13;
    pub const OP_LESS: u8 = 0x14;
    pub const OP_LOC: u8 = 0x25;
    pub const OP_LOC_POP: u8 = 0x15;
    pub const OP_LOC_PUSH: u8 = 0x16;
    pub const OP_MULTIPLY: u8 = 0x17;
    pub const OP_NEGATE: u8 = 0x18;
    pub const OP_NEW: u8 = 0x19;
    pub const OP_NOT: u8 = 0x1A;
    pub const OP_OR: u8 = 0x1B;
    pub const OP_PARALLEL: u8 = 0x1C;
    pub const OP_POP: u8 = 0x1D;
    pub const OP_POP_N: u8 = 0x1E;
    pub const OP_RETURN: u8 = 0x1F;
    pub const OP_SET_GLOBAL: u8 = 0x20;
    pub const OP_SET_LOCAL: u8 = 0x21;
    pub const OP_SUBSTRACT: u8 = 0x22;
    pub const OP_TRUE: u8 = 0x23;
    pub const OP_UNIT: u8 = 0x24;
}

#[derive(Clone)]
pub struct ClassMut {
    pub name: String,
    pub properties: HashMap<String, String>,
    pub methods: HashMap<String, FunctionMut>,
}

impl ClassMut {
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

    /* TIM */
    /// **Edited to work with custom Heap.**
    ///
    /// Freezes the class onto the heap, effectively freezing all its functions on it with the class definition on top.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap to freeze the class on.
    /// 
    /// **Returns**  
    /// The heap-frozen Class if everything went alright, or a HeapError otherwise.
    pub fn freeze(
        self,
        heap: &mut Heap<Object>,
    ) -> Result<Class, HeapError> {
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
    /*******/
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

#[derive(Clone)]
pub struct FunctionMut {
    pub arity: u8,
    pub chunk: ChunkMut,
    pub name: String,
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

impl FunctionMut {
    ///
    ///
    ///
    pub fn main(chunk: ChunkMut) -> Self {
        Self {
            arity: 0,
            chunk,
            name: String::from("main"),
        }
    }

    ///
    ///
    ///
    pub fn new(
        name: String,
        arity: u8,
        chunk: ChunkMut,
    ) -> Self {
        Self { arity, chunk, name }
    }

    /* TIM */
    /// **Edited: Now returning a HeapError**
    ///
    /// Freezes the function onto the heap.
    /// 
    /// **Returns**  
    /// A frozen Function if we could freeze it, or a HeapError otherwise.
    pub fn freeze(
        self,
        heap: &mut Heap<Object>,
    ) -> Result<objects::Function, HeapError> {
        Ok(Function::new(self.name, self.arity, self.chunk.freeze(heap)?))
    }
    /*******/
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Bytes,
    pub constants: Vec<Slot>,
}

impl Chunk {
    ///
    ///
    ///
    pub fn unfreeze(
        self,
        heap: &Heap<Object>,
    ) -> ChunkMut {
        let constants = self.constants.into_iter().map(|s| s.into_value(heap)).collect();

        ChunkMut::new(BytesMut::from(&self.code[..]), constants)
    }

    ///
    ///
    ///
    pub fn disassemble(&self) -> Result<String> {
        let mut result = String::new();
        let mut skip = 0;

        for (offset, instruction) in self.code.iter().enumerate() {
            if skip > 0 {
                skip -= 1;
                continue;
            }

            use opcodes::*;
            write!(result, "{:04} ", offset)?;
            match *instruction {
                OP_CONSTANT => {
                    constant_instruction("OP_CONSTANT", self, offset, &mut result);
                    skip = 1;
                }
                OP_ADD => {
                    writeln!(result, "OP_ADD")?;
                }
                OP_AND => {
                    writeln!(result, "OP_AND")?;
                }
                OP_DIVIDE => {
                    writeln!(result, "OP_DIVIDE")?;
                }
                OP_EQUAL => {
                    writeln!(result, "OP_EQUAL")?;
                }
                OP_FALSE => {
                    writeln!(result, "OP_FALSE")?;
                }
                OP_GREATER => {
                    writeln!(result, "OP_GREATER")?;
                }
                OP_LESS => {
                    writeln!(result, "OP_LESS")?;
                }
                OP_MULTIPLY => {
                    writeln!(result, "OP_MULTIPLY")?;
                }
                OP_NEGATE => {
                    writeln!(result, "OP_NEGATE")?;
                }
                OP_NOT => {
                    writeln!(result, "OP_NOT")?;
                }
                OP_OR => {
                    writeln!(result, "OP_OR")?;
                }
                OP_POP => {
                    writeln!(result, "OP_POP")?;
                }
                OP_POP_N => {
                    byte_instruction("OP_POP_N", self, offset, &mut result);
                    skip = 1;
                }
                OP_RETURN => {
                    writeln!(result, "OP_RETURN")?;
                }
                OP_SUBSTRACT => {
                    writeln!(result, "OP_SUBSTRACT")?;
                }
                OP_TRUE => {
                    writeln!(result, "OP_TRUE")?;
                }
                OP_UNIT => {
                    writeln!(result, "OP_UNIT")?;
                }
                OP_LOC => {
                    writeln!(result, "OP_LOC")?;
                }
                OP_INDEX => {
                    writeln!(result, "OP_INDEX")?;
                }
                OP_LOC_PUSH => {
                    writeln!(result, "OP_LOC_PUSH")?;
                }
                OP_LOC_POP => {
                    writeln!(result, "OP_LOC_POP")?;
                }
                OP_DOT => {
                    constant_instruction("OP_DOT", self, offset, &mut result);
                    skip = 1;
                }
                OP_ARRAY => {
                    byte_instruction("OP_ARRAY", self, offset, &mut result);
                    skip = 1;
                }
                OP_PARALLEL => {
                    byte_instruction("OP_PARALLEL", self, offset, &mut result);
                    skip = 1;
                }
                OP_NEW => {
                    byte_instruction("OP_NEW", self, offset, &mut result);
                    skip = 1;
                }
                OP_CALL => {
                    byte_instruction("OP_CALL", self, offset, &mut result);
                    skip = 1;
                }
                OP_JUMP_IF_FALSE => {
                    jump_instruction("OP_JUMP_IF_FALSE", 1, self, offset, &mut result);
                    skip = 2;
                }
                OP_JUMP => {
                    jump_instruction("OP_JUMP", 1, self, offset, &mut result);
                    skip = 2;
                }
                OP_JUMP_BACK => {
                    jump_instruction("OP_JUMP_BACK", -1, self, offset, &mut result);
                    skip = 2;
                }
                OP_DEFINE_GLOBAL => {
                    constant_instruction("OP_DEFINE_GLOBAL", self, offset, &mut result);
                    skip = 1;
                }
                OP_GET_GLOBAL => {
                    constant_instruction("OP_GET_GLOBAL", self, offset, &mut result);
                    skip = 1;
                }
                OP_GET_LOCAL => {
                    byte_instruction("OP_GET_LOCAL", self, offset, &mut result);
                    skip = 1;
                }
                OP_GET_METHOD => {
                    constant_instruction("OP_GET_METHOD", self, offset, &mut result);
                    skip = 1;
                }
                OP_GET_PROPERTY => {
                    constant_instruction("OP_GET_PROPERTY", self, offset, &mut result);
                    skip = 1;
                }
                OP_SET_GLOBAL => {
                    byte_instruction("OP_SET_GLOBAL", self, offset, &mut result);
                    skip = 1;
                }
                OP_SET_LOCAL => {
                    byte_instruction("OP_SET_LOCAL", self, offset, &mut result);
                    skip = 1;
                }
                OP_CLASS => {
                    constant_instruction("OP_CLASS", self, offset, &mut result);
                    skip = 1;
                }
                OP_IMPORT => {
                    constant_instruction("OP_IMPORT", self, offset, &mut result);
                    skip = 1;
                }
                0x00 | 0x28..=u8::MAX => {
                    unreachable!()
                }
            }
        }

        Ok(result)
    }
}

///
///
///
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

///
///
///
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

///
///
///
fn byte_instruction(
    name: &str,
    chunk: &Chunk,
    offset: usize,
    result: &mut String,
) {
    let slot = chunk.code[offset + 1];
    writeln!(result, "{:<16} {:4} | ", name, slot).unwrap();
}

#[derive(Clone, Debug)]
pub struct ChunkMut {
    pub code: BytesMut,
    pub constants: Vec<Value>,
}

impl Default for ChunkMut {
    fn default() -> Self {
        Self {
            code: BytesMut::default(),
            constants: Vec::default(),
        }
    }
}

impl ChunkMut {
    ///
    ///
    ///
    pub fn new(
        code: BytesMut,
        constants: Vec<Value>,
    ) -> Self {
        ChunkMut { code, constants }
    }

    /* TIM */
    /// **Edited to work with custom Heap.**
    ///
    /// Freezes a function body / chunk of code on the heap.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap to freeze the class on.
    /// 
    /// **Returns**  
    /// The heap-frozen Chunk if everything went alright, or a HeapError otherwise.
    pub fn freeze(
        self,
        heap: &mut Heap<Object>,
    ) -> Result<Chunk, HeapError> {
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

    ///
    ///
    ///
    pub fn write<B: Into<u8>>(
        &mut self,
        byte: B,
    ) {
        self.code.put_u8(byte.into());
    }

    ///
    ///
    ///
    pub fn write_pair<B1: Into<u8>, B2: Into<u8>>(
        &mut self,
        byte1: B1,
        byte2: B2,
    ) {
        self.code.put_u8(byte1.into());
        self.code.put_u8(byte2.into());
    }

    ///
    ///
    ///
    pub fn write_bytes(
        &mut self,
        bytes: &[u8],
    ) {
        self.code.extend(bytes);
    }

    ///
    ///
    ///
    pub fn add_constant(
        &mut self,
        value: Value,
    ) -> u8 {
        self.constants.push(value);

        (self.constants.len() as u8) - 1
    }
}
