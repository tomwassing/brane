use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FResult, Write};
use std::usize;

use fnv::FnvHashMap;
use specifications::common::SpecClass;
use specifications::common::Value;

use crate::builtins::BuiltinFunction;
use crate::bytecode::{BytecodeError, ClassMut};
use crate::heap::{Handle, Heap, HeapError};
use crate::objects::Array;
use crate::objects::Instance;
use crate::objects::{Object, ObjectError};


/***** CONSTANTS *****/
const STACK_MAX: usize = 256;





/***** UNIT TESTS *****/
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_pop() {
        let mut stack = Stack::default();
        stack.push(Slot::Integer(1));
        stack.push(Slot::Integer(2));
        stack.push(Slot::Integer(3));

        stack.copy_pop(0);

        assert_eq!(stack.len(), 2);
        assert_eq!(stack.pop_integer(), Ok(2));
        assert_eq!(stack.pop_integer(), Ok(3));
    }

    #[test]
    fn test_copy_push() {
        let mut stack = Stack::default();
        stack.push(Slot::Integer(1));
        stack.push(Slot::Integer(2));

        stack.copy_push(0);

        assert_eq!(stack.len(), 3);
        assert_eq!(stack.pop_integer(), Ok(1));
        assert_eq!(stack.pop_integer(), Ok(2));
        assert_eq!(stack.pop_integer(), Ok(1));
    }
}





/***** ERRORS *****/
/// Enum that provides errors for stack-related operations
#[derive(Debug, PartialEq)]
pub enum StackError {
    /// Error for when we expected one type to be on top of the stack, but found another
    UnexpectedType{ got: String, expected: String },
    /// Error for when we expected the stack to contain something, but it didn't
    EmptyStackError{ what: String },
    /// Error for when an index went out-of-bounds for the stack
    OutOfBoundsError{ i: usize, capacity: usize },
    /// Error for when we see an optimized constant, but we do not expect it
    NotUsingConstOpts{ slot: Slot },
    /// An Array could not resolve its subtype
    ArrayTypeError{ err: ObjectError },

    /// Error for when an allocation on the Heap failed
    HeapAllocError{ what: String, err: HeapError },
    /// Error for when we could not freeze something on the Heap
    HeapFreezeError{ what: String, err: BytecodeError },
}

impl std::fmt::Display for StackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StackError::UnexpectedType{ got, expected } => write!(f, "Expected to find value of type {} on top of stack, but got {}", expected, got),
            StackError::EmptyStackError{ what }         => write!(f, "Expected to find {}, but stack is empty", what),
            StackError::OutOfBoundsError{ i, capacity } => write!(f, "Index {} is out-of-bounds for stack of {} slots", i, capacity),
            StackError::NotUsingConstOpts{ slot }       => write!(f, "Encountered optimized constant '{}', but optimized constants are disabled in this Stack", slot),
            StackError::ArrayTypeError{ err }           => write!(f, "{}", err),

            StackError::HeapAllocError{ what, err }  => write!(f, "Could not allocate {} on the heap: {}", what, err),
            StackError::HeapFreezeError{ what, err } => write!(f, "Could not freeze {} on the heap: {}", what, err),
        }
    }
}

impl std::error::Error for StackError {}





/***** LIBRARY STRUCTS *****/
/// Defines a single Slot in the stack.
#[derive(Clone, Debug)]
pub enum Slot {
    /// A built-in function
    BuiltIn(BuiltinFunction),
    /// Optimization for Integer(-1)
    ConstMinusOne,
    /// Optimization for Integer(-2)
    ConstMinusTwo,
    /// Optimization for Integer(1)
    ConstOne,
    /// Optimization for Integer(2)
    ConstTwo,
    /// Optimization for Integer(0)
    ConstZero,
    /// A boolean (false)
    False,
    /// An integral number
    Integer(i64),
    /// A real number
    Real(f64),
    /// A boolean (true)
    True,
    /// Short for 'unitialized'; basically void.
    Unit,
    /// An Object, which lives on the Heap.
    Object(Handle<Object>),
}

impl Slot {
    /// **Edited: Changed to use the custom Heap implementation.**
    ///
    /// Tries to get the object behind this Slot if it is one.
    /// 
    /// **Returns**  
    /// A Handle to the object if we are an object, or None otherwise.
    #[inline]
    pub fn as_object(&self) -> Option<Handle<Object>> {
        match self {
            Slot::Object(handle) => Some(handle.clone()),
            _                    => None,
        }
    }

    /// **Edited: Changed to use the custom Heap implementation.**
    ///
    /// Constructor for the Slot that takes a Value as basis.
    /// 
    /// **Arguments**
    ///  * `value`: The Value to construct this slot with.
    ///  * `globals`: The list of global variables to get types from.
    ///  * `heap`: The Heap to allocate stuff on that won't go onto the stack but is needed by objects.
    /// 
    /// **Returns**  
    /// The new Slot object if we could do all the allocations and junk, or a StackError otherwise.
    pub fn from_value(
        value: Value,
        globals: &FnvHashMap<String, Slot>,
        heap: &mut Heap<Object>,
    ) -> Result<Self, StackError> {
        match value {
            Value::Unicode(s) => {
                // Convert to a' Object
                let string = Object::String(s);
                // Try to allocate on the heap
                match heap.alloc(string) {
                    Ok(handle)  => Ok(Slot::Object(handle)),
                    Err(reason) => Err(StackError::HeapAllocError{ what: "a String".to_string(), err: reason }),
                }
            }
            Value::Boolean(b) => match b {
                false => Ok(Slot::False),
                true => Ok(Slot::True),
            },
            Value::Integer(i) => Ok(Slot::Integer(i)),
            Value::Real(r) => Ok(Slot::Real(r)),
            Value::Unit => Ok(Slot::Unit),
            Value::FunctionExt(f) => {
                // Convert to a' Object
                let function = Object::FunctionExt(f);
                // Try to allocate on the heap
                match heap.alloc(function) {
                    Ok(handle)  => Ok(Slot::Object(handle)),
                    Err(reason) => Err(StackError::HeapAllocError{ what: "an external Function (FunctionExt)".to_string(), err: reason }),
                }
            }
            Value::Class(c) => {
                // Freeze the class on the heap
                let class: ClassMut = c.into();
                let class = match class.freeze(heap) {
                    Ok(c)       => c,
                    Err(reason) => { return Err(StackError::HeapFreezeError{ what: "a Class".to_string(), err: reason }); }
                };

                // Now try to allocate the frozen class
                match heap.alloc(Object::Class(class)) {
                    Ok(handle)  => Ok(Slot::Object(handle)),
                    Err(reason) => Err(StackError::HeapAllocError{ what: "a Class".to_string(), err: reason }),
                }
            }
            Value::Struct { data_type, properties } => {
                // First put all values on the heap
                let mut i_properties = FnvHashMap::default();
                for (name, value) in properties {
                    i_properties.insert(name.clone(), Slot::from_value(value.clone(), globals, heap)?);
                }

                // Next, try to get the global for the class definition
                let i_class = globals
                    .get(&data_type)
                    .unwrap_or_else(|| panic!("Expecting '{}' to be loaded as a global, but it isn't; this should never happen!", data_type))
                    .as_object()
                    .unwrap_or_else(|| panic!("Expecting '{}' to be an Object, but it isn't; this should never happen!", data_type));

                // Create the instance of this struct/class
                let instance = Instance::new(i_class, i_properties);
                let instance = Object::Instance(instance);
                match heap.alloc(instance) {
                    Ok(handle)  => Ok(Slot::Object(handle)),
                    Err(reason) => Err(StackError::HeapAllocError{ what: "an Instance of a Struct".to_string(), err: reason }),
                }
            }
            Value::Array { entries, .. } => {
                // Put the entries on the stack first
                let mut new_entries: Vec<Slot> = Vec::with_capacity(entries.len());
                for e in entries {
                    new_entries.push(Slot::from_value(e, globals, heap)?);
                }

                // Put the Array itself on the stack
                let array = Object::Array(Array::new(new_entries).map_err(|err| StackError::ArrayTypeError{ err })?);
                match heap.alloc(array) {
                    Ok(handle)  => Ok(Slot::Object(handle)),
                    Err(reason) => Err(StackError::HeapAllocError{ what: "an Array".to_string(), err: reason }),
                }
            }
            todo => {
                panic!("Cannot put value of type '{}' ('{}') in a Slot", todo.data_type(), todo);
            }
        }
    }



    /// **Edited: changed to use custom Heap implementation.**
    /// 
    /// Consumes the Slot into a Value.
    pub fn into_value(self) -> Value {
        match self {
            Slot::BuiltIn(_)    => Value::Unit,
            Slot::ConstMinusOne => Value::Integer(-1),
            Slot::ConstMinusTwo => Value::Integer(-2),
            Slot::ConstOne      => Value::Integer(1),
            Slot::ConstTwo      => Value::Integer(2),
            Slot::ConstZero     => Value::Integer(0),
            Slot::False         => Value::Boolean(false),
            Slot::Integer(i)    => Value::Integer(i),
            Slot::Real(r)       => Value::Real(r),
            Slot::True          => Value::Boolean(true),
            Slot::Unit          => Value::Unit,
            Slot::Object(h) => match h.get() {
                Object::Array(a) => {
                    // Convert the Object-Array to a Value-Array
                    let data_type = a.element_type.clone();
                    let entries = a.elements.iter().map(|s| s.clone().into_value()).collect();
                    Value::Array { data_type, entries }
                }
                Object::Class(c) => {
                    // Convert the Object-Class to a Value-Class
                    let class = c.clone().unfreeze();
                    let class: SpecClass = class.into();
                    Value::Class(class)
                }
                Object::Function(_)    => panic!("Cannot convert function to value."),
                Object::FunctionExt(f) => Value::FunctionExt(f.clone()),
                Object::Instance(i)    => {
                    // Convert the Object-Instance to a Value-Struct
                    // Collect the type name
                    let data_type = i.class.get().as_class().expect("Instance parent is not a Class").name.clone();
                    // Collect a list of properties
                    let mut properties = HashMap::new();
                    for (name, slot) in &i.properties {
                        properties.insert(name.clone(), slot.clone().into_value());
                    }
                    // Return the Struct
                    Value::Struct { data_type, properties }
                }
                Object::String(s) => Value::Unicode(s.clone()),
            },
        }
    }



    /// Returns a string representation of the data type of this slot.
    pub fn data_type(&self) -> String {
        match self {
            Slot::BuiltIn(_)    => "BuiltIn".to_string(),
            Slot::ConstMinusOne => "Integer".to_string(),
            Slot::ConstMinusTwo => "Integer".to_string(),
            Slot::ConstOne      => "Integer".to_string(),
            Slot::ConstTwo      => "Integer".to_string(),
            Slot::ConstZero     => "Integer".to_string(),
            Slot::False         => "Boolean".to_string(),
            Slot::Integer(_)    => "Integer".to_string(),
            Slot::Real(_)       => "Real".to_string(),
            Slot::True          => "Boolean".to_string(),
            Slot::Unit          => "Unit".to_string(),
            Slot::Object(h)     => match h.get() {
                Object::Array(a)       => format!("Array<{}>", a.element_type),
                Object::Class(c)       => format!("Class<{}>", c.name),
                Object::Function(f)    => format!("Function<{}>", f.name),
                Object::FunctionExt(f) => format!("FunctionExt<{}; {}>", f.name, f.kind),
                Object::Instance(i)    => format!("Instance<{}>", i.class.get().as_class().expect("Instance parent is not a Class").name),
                Object::String(_)      => "String".to_string(),
            },
        }
    }
}

impl Display for Slot {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        let display = match self {
            Slot::BuiltIn(code) => format!("builtin<{:#04x}>", *code as u8), // TODO: More information, func or class, name?
            Slot::ConstMinusOne => String::from("-1"),
            Slot::ConstMinusTwo => String::from("-2"),
            Slot::ConstOne => String::from("1"),
            Slot::ConstTwo => String::from("2"),
            Slot::ConstZero => String::from("0"),
            Slot::False => String::from("false"),
            Slot::Integer(i) => format!("{}", i),
            Slot::Real(r) => format!("{}", r),
            Slot::True => String::from("true"),
            Slot::Unit => String::from("unit"),
            Slot::Object(h) => match h.get() {
                Object::Array(a) => format!("array<{}>", a.element_type),
                Object::Class(c) => format!("class<{}>", c.name),
                Object::Function(f) => format!("function<{}>", f.name),
                Object::FunctionExt(f) => format!("function<{}; {}>", f.name, f.kind),
                Object::Instance(i) => format!("instance<{}>", i.class.get().as_class().expect("Instance parent is not a Class").name),
                Object::String(s) => format!("{:?}", s),
            },
        };

        write!(f, "{}", display)
    }
}

impl PartialEq for Slot {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        match (self, other) {
            (Slot::BuiltIn(lhs), Slot::BuiltIn(rhs)) => lhs == rhs,
            (Slot::ConstMinusOne, Slot::ConstMinusOne) => true,
            (Slot::ConstMinusTwo, Slot::ConstMinusTwo) => true,
            (Slot::ConstOne, Slot::ConstOne) => true,
            (Slot::ConstTwo, Slot::ConstTwo) => true,
            (Slot::ConstZero, Slot::ConstZero) => true,
            (Slot::False, Slot::False) => true,
            (Slot::Integer(lhs), Slot::Integer(rhs)) => lhs == rhs,
            (Slot::Real(lhs), Slot::Real(rhs)) => lhs == rhs,
            (Slot::True, Slot::True) => true,
            (Slot::Unit, Slot::Unit) => true,
            (Slot::Object(lhs), Slot::Object(rhs)) => lhs == rhs,
            _ => false,
        }
    }
}

impl PartialOrd for Slot {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        match (self, other) {
            (Slot::Integer(lhs), Slot::Integer(rhs)) => lhs.partial_cmp(rhs),
            (Slot::Integer(lhs), Slot::Real(rhs)) => (*lhs as f64).partial_cmp(rhs),
            (Slot::Real(lhs), Slot::Real(rhs)) => lhs.partial_cmp(rhs),
            (Slot::Real(lhs), Slot::Integer(rhs)) => lhs.partial_cmp(&(*rhs as f64)),
            _ => None,
        }
    }
}



/// Implements the actual Stack.
#[derive(Debug)]
pub struct Stack {
    /// The internal data array that is the Stack.
    inner: Vec<Slot>,
    /// Whether or not to use constant optimizations.
    use_const: bool,
}

impl Default for Stack {
    fn default() -> Self {
        Self::new(STACK_MAX, true)
    }
}

impl Display for Stack {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        let mut display = String::from("         ");
        self.inner.iter().for_each(|v| write!(display, "[ {} ]", v).unwrap());

        write!(f, "{}", display)
    }
}

impl Stack {
    /// Constructor for the Stack.
    /// 
    /// **Arguments**
    ///  * `size`: The size of the stack. Cannot be changed later.
    ///  * `use_const`: Whether to use the constants optimization or not.
    pub fn new(
        size: usize,
        use_const: bool,
    ) -> Self {
        Self {
            inner: Vec::with_capacity(size),
            use_const,
        }
    }



    /// Returns the Slot at the given index in the stack.  
    /// Note that this function _will_ panic if it goes out-of-bounds.
    /// 
    /// **Arguments**
    ///  * `index`: The index of the Slot to get.
    /// 
    /// **Returns**  
    /// The requested Slot.
    pub fn get(&self, index: usize) -> &Slot {
        if index >= self.inner.len() { panic!("Index {} is out-of-bounds for Stack of size {}", index, self.inner.len()); }
        &self.inner[index]
    }

    /// **Edited: Changed to work with custom Heap.**
    ///
    /// Gets the slot at the given index as an Object.  
    /// Will panic if the given index is out-of-bounds.
    /// 
    /// **Arguments**
    ///  * `index`: The index of the slot to retrieve. Will throw errors if it is out-of-bounds.
    /// 
    /// **Returns**  
    /// Returns a Handle to the Object that we wanted to retrieve if index pointer to an Object, or a StackError otherwise.
    pub fn get_object(&self, index: usize) -> Result<Handle<Object>, StackError> {
        if index >= self.inner.len() { panic!("Index {} is out-of-bounds for Stack of size {}", index, self.inner.len()); }
        match &self.inner[index] {
            Slot::Object(h) => Ok(h.clone()),
            _               => Err(StackError::UnexpectedType{ got: self.inner[index].data_type(), expected: "Object".to_string() }),
        }
    }



    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top boolean on the stack without popping it.  
    /// Will panic if the Stack is empty.
    /// 
    /// **Returns**  
    /// The boolean's value if successfull, or a StackError otherwise.
    #[inline]
    pub fn peek_boolean(&mut self) -> Result<bool, StackError> {
        // Get the top value
        let top = match self.inner.last() {
            Some(top) => top,
            None      => { panic!("Cannot peek on empty Stack."); }
        };

        // Match the (hopefully) boolean value with the top one
        match top {
            Slot::False => Ok(false),
            Slot::True  => Ok(true),
            _           => Err(StackError::UnexpectedType{ expected: "Boolean".to_string(), got: top.data_type().to_string() }),
        }
    }



    /// Tries to return the top slot of the stack.
    /// 
    /// **Returns**  
    /// The Slot if there is one, or else None if the Stack is empty.
    #[inline]
    pub fn try_pop(&mut self) -> Option<Slot> {
        self.inner.pop()
    }

    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top slot of the stack.  
    /// Will panic if the Stack is empty.
    /// 
    /// **Returns**  
    /// The slot with its value if successfull, or a StackError otherwise. Note that it will never fail if you know the stack has at least one element, so you can safely call .unwrap() in that case.
    #[inline]
    pub fn pop(&mut self) -> Result<Slot, StackError> {
        // Try to get the last value
        let slot = match self.inner.pop() {
            Some(top) => top,
            None      => { panic!("Cannot pop from empty Stack."); }
        };

        // We can return the slot immediately if we won't see any constants anyway
        if !self.use_const {
            return Ok(slot);
        }

        // Otherwise, wrap const-slots in integers and return that
        match slot {
            Slot::ConstMinusOne => Ok(Slot::Integer(-1)),
            Slot::ConstMinusTwo => Ok(Slot::Integer(-2)),
            Slot::ConstOne      => Ok(Slot::Integer(1)),
            Slot::ConstTwo      => Ok(Slot::Integer(2)),
            Slot::ConstZero     => Ok(Slot::Integer(0)),
            slot                => Ok(slot),
        }
    }

    /// Removes the given element from the stack.  
    /// The removed element will be replaced with the top element of the stack (effectively popping that as well).
    ///
    /// **Arguments**
    ///  * `index`: The index of the element to copy and pop.
    /// 
    /// **Returns**  
    /// The requested Slot.
    #[inline]
    pub fn copy_pop(&mut self, index: usize) {
        self.inner.swap_remove(index);
    }

    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top value of the stack as a boolean.  
    /// Will panic if the Stack is empty.
    /// 
    /// **Returns**  
    /// The boolean value if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_boolean(&mut self) -> Result<bool, StackError> {
        // Try to get the top value
        let slot = match self.inner.pop() {
            Some(top) => top,
            None      => { panic!("Cannot pop from empty Stack."); }
        };

        // Match on the type
        match slot {
            Slot::False => Ok(false),
            Slot::True  => Ok(true),
            _           => Err(StackError::UnexpectedType{ expected: "Boolean".to_string(), got: slot.data_type().to_string() }),
        }
    }

    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top value of the stack as an integer.  
    /// Will panic if the Stack is empty.
    /// 
    /// **Returns**  
    /// The value of the integer if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_integer(&mut self) -> Result<i64, StackError> {
        // Try to get the top value
        let slot = match self.inner.pop() {
            Some(top) => top,
            None      => { panic!("Cannot pop from empty Stack."); }
        };

        // Match on the type
        match slot {
            Slot::ConstMinusTwo => if self.use_const { Ok(-2) } else { Err(StackError::NotUsingConstOpts{ slot }) },
            Slot::ConstMinusOne => if self.use_const { Ok(-1) } else { Err(StackError::NotUsingConstOpts{ slot }) },
            Slot::ConstZero     => if self.use_const { Ok(0) } else { Err(StackError::NotUsingConstOpts{ slot }) },
            Slot::ConstOne      => if self.use_const { Ok(1) } else { Err(StackError::NotUsingConstOpts{ slot }) },
            Slot::ConstTwo      => if self.use_const { Ok(2) } else { Err(StackError::NotUsingConstOpts{ slot }) },
            Slot::Integer(n)    => if self.use_const { Ok(n) } else { Err(StackError::NotUsingConstOpts{ slot }) },
            _                   => Err(StackError::UnexpectedType{ expected: "Integer".to_string(), got: slot.data_type().to_string() }),
        }
    }

    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top value of the stack as a real number.  
    /// Will panic if the Stack is empty.
    /// 
    /// **Returns**  
    /// The value of the real number if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_real(&mut self) -> Result<f64, StackError> {
        // Try to get the top value
        let slot = match self.inner.pop() {
            Some(top) => top,
            None      => { panic!("Cannot pop from empty Stack."); }
        };

        // Match on the type
        match slot {
            Slot::Real(n) => Ok(n),
            _             => Err(StackError::UnexpectedType{ expected: "Real".to_string(), got: slot.data_type().to_string() }),
        }
    }

    /// **Edited: Changed to return a StackError instead of panicking + now working with the new custom Heap.**
    ///
    /// Returns the top value of the stack as an object.  
    /// Will panic if the Stack is empty.
    /// 
    /// **Returns**  
    /// A handle to the object if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_object(&mut self) -> Result<Handle<Object>, StackError> {
        // Try to get the top value
        let slot = match self.inner.pop() {
            Some(top) => top,
            None      => { panic!("Cannot pop from empty Stack."); }
        };

        // Match on the type
        match slot {
            Slot::Object(h) => Ok(h),
            _               => Err(StackError::UnexpectedType{ expected: "Object".to_string(), got: slot.data_type().to_string() }),
        }
    }

    /// **Edited: Changed to return a StackError instead of panicking + now working with the new custom Heap.**
    ///
    /// Returns the top value of the stack as a Unit.  
    /// Will panic if the Stack is empty.
    /// 
    /// **Returns**  
    /// The Unit (i.e., nothing) if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_unit(&mut self) -> Result<(), StackError> {
        // Try to get the top value
        let slot = match self.inner.pop() {
            Some(top) => top,
            None      => { panic!("Cannot pop from empty Stack."); }
        };

        // Match on the type
        match slot {
            Slot::Unit => Ok(()),
            _          => Err(StackError::UnexpectedType{ expected: "Unit".to_string(), got: slot.data_type().to_string() }),
        }
    }



    /// Tries to push the given Option<Slot> on the stack.  
    /// If the Option is None, then nothing is pushed.
    /// 
    /// **Arguments**
    ///  * `slot`: The maybe-Slot to push.
    #[inline]
    pub fn try_push(&mut self, slot: Option<Slot>) {
        // Only push if there is something to push
        if let Some(slot) = slot {
            self.inner.push(slot)
        }
    }

    /// Pushes the given Slot on top of the stack.
    ///
    /// **Arguments**
    ///  * `slot`: The Slot to push.
    #[inline]
    pub fn push(&mut self, slot: Slot) {
        self.inner.push(slot);
    }

    /// Copies the element at the given index to the top of the stack.
    /// 
    /// **Arguments**
    ///  * `index`: The index of the Slot to copy and push to the top of the stack.
    #[inline]
    pub fn copy_push(&mut self, index: usize) {
        if index >= self.inner.len() { panic!("Index {} is out-of-bounds for Stack of size {}", index, self.inner.len()); }

        // Copy the element
        let elem = self.inner[index].clone();

        // Push it
        self.inner.push(elem);
    }

    /// Pushes the given boolean on top of the Stack.
    /// 
    /// **Arguments**
    ///  * `boolean`: The boolean value to push.
    #[inline]
    pub fn push_boolean(&mut self, boolean: bool) {
        // Convert the value to a Slot
        let boolean = match boolean {
            false => Slot::False,
            true => Slot::True,
        };

        // Push it
        self.inner.push(boolean);
    }

    /// Pushes the given integer on top of the Stack.
    /// 
    /// **Arguments**
    ///  * `integer`: The integer value to push.
    #[inline]
    pub fn push_integer(&mut self, integer: i64) {
        // Map the integer to a Slot
        let integer = if self.use_const {
            // Use optimized constants as well
            match integer {
                -2 => Slot::ConstMinusTwo,
                -1 => Slot::ConstMinusOne,
                0 => Slot::ConstZero,
                1 => Slot::ConstOne,
                2 => Slot::ConstTwo,
                n => Slot::Integer(n),
            }
        } else {
            Slot::Integer(integer)
        };

        // Push it
        self.inner.push(integer);
    }

    /// Pushes the given real number on top of the Stack.
    /// 
    /// **Arguments**
    ///  * `real`: The real value to push.
    #[inline]
    pub fn push_real(&mut self, real: f64) {
        self.inner.push(Slot::Real(real));
    }

    /// **Edited: now working with the custom Heap.**
    ///
    /// Pushes a given Object handle onto the stack.
    /// 
    /// **Arguments**
    ///  * `object`: The handle to put on there.
    #[inline]
    pub fn push_object(&mut self, object: Handle<Object>) {
        self.inner.push(Slot::Object(object));
    }

    /// Pushes a Unit onto the Stack.
    #[inline]
    pub fn push_unit(&mut self) {
        self.inner.push(Slot::Unit);
    }



    /// Clears the stack completely.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Clears the stack from the given index onwards.
    /// 
    /// **Arguments**
    ///  * `index`: The index of the first item to remove; anything before it will be kept.
    #[inline]
    pub fn clear_from(&mut self, index: usize) {
        self.inner.truncate(index)
    }



    /// Returns the number of slots currently populated on the Stack.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns if the Stack is empty or not.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
