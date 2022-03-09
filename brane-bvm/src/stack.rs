use crate::bytecode::ClassMut;
use crate::objects::Array;
use crate::objects::Instance;
use crate::objects::Object;
use crate::builtins::BuiltinFunction;
use crate::heap::{Handle, Heap, HeapError};
use fnv::FnvHashMap;
use specifications::common::SpecClass;
use specifications::common::Value;
use std::collections::HashMap;
use std::fmt::Write;
use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
    usize,
};


/* TIM */
/// Enum that provides errors for stack-related operations
#[derive(Debug, PartialEq)]
pub enum StackError {
    /// Error for when we expected one type to be on top of the stack, but found another
    UnexpectedType{ got: String, expected: String },
    /// Error for when we expected the stack to contain something, but it didn't
    EmptyStackError{ what: String },
    /// Error for when an index went out-of-bounds for the stack
    OutOfBoundsError{ i: usize, capacity: usize },

    /// Error for when an allocation on the Heap failed
    HeapAllocError{ what: String, err: HeapError },
    /// Error for when we could not freeze something on the Heap
    HeapFreezeError{ what: String, err: HeapError },
}

impl std::fmt::Display for StackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StackError::UnexpectedType{ got, expected } => write!(f, "Expected to find value of type {} on top of stack, but got {}", expected, got),
            StackError::EmptyStackError{ what }         => write!(f, "Expected to find {}, but stack is empty", what),
            StackError::OutOfBoundsError{ i, capacity } => write!(f, "Index {} is out-of-bounds for stack of {} slots", i, capacity),

            StackError::HeapAllocError{ what, err }  => write!(f, "Could not allocate {} on the heap: {}", what, err),
            StackError::HeapFreezeError{ what, err } => write!(f, "Could not allocate {} on the heap: {}", what, err),
        }
    }
}

impl std::error::Error for StackError {}
/*******/


const STACK_MAX: usize = 256;

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Slot {
    BuiltIn(BuiltinFunction),
    ConstMinusOne,
    ConstMinusTwo,
    ConstOne,
    ConstTwo,
    ConstZero,
    False,
    Integer(i64),
    Real(f64),
    True,
    Unit,
    /* TIM */
    Object(Handle),
    /*******/
}

impl Slot {
    /* TIM */
    /// **Edited: Changed to use the custom Heap implementation.**
    ///
    /// Tries to get the object behind this Slot if it is one.
    /// 
    /// **Returns**  
    /// A Handle to the object if we are an object, or None otherwise.
    #[inline]
    pub fn as_object(&self) -> Option<Handle> {
        match self {
            Slot::Object(handle) => Some(*handle),
            _                    => None,
        }
    }
    /*******/

    /* TIM */
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
                let array = Object::Array(Array::new(new_entries));
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
    /*******/

    ///
    ///
    ///
    pub fn into_value(
        self,
        heap: &Heap<Object>,
    ) -> Value {
        match self {
            Slot::BuiltIn(_) => {
                // panic!("Cannot convert built-in to value.")

                Value::Unit
            }
            Slot::ConstMinusOne => Value::Integer(-1),
            Slot::ConstMinusTwo => Value::Integer(-2),
            Slot::ConstOne => Value::Integer(1),
            Slot::ConstTwo => Value::Integer(2),
            Slot::ConstZero => Value::Integer(0),
            Slot::False => Value::Boolean(false),
            Slot::Integer(i) => Value::Integer(i),
            Slot::Real(r) => Value::Real(r),
            Slot::True => Value::Boolean(true),
            Slot::Unit => Value::Unit,
            Slot::Object(h) => match heap.get(h).unwrap() {
                Object::Array(a) => {
                    let data_type = a.element_type.clone();
                    let entries = a.elements.iter().map(|s| s.into_value(heap)).collect();

                    Value::Array { data_type, entries }
                }
                Object::Class(c) => {
                    let class = c.clone().unfreeze(heap);
                    let class: SpecClass = class.into();
                    Value::Class(class)
                }
                Object::Function(_) => panic!("Cannot convert function to value."),
                Object::FunctionExt(f) => Value::FunctionExt(f.clone()),
                Object::Instance(i) => {
                    let class = heap.get(i.class).expect("").as_class().expect("");
                    let data_type = class.name.clone();

                    let mut properties = HashMap::new();

                    for (name, slot) in &i.properties {
                        properties.insert(name.clone(), (*slot).into_value(heap));
                    }

                    Value::Struct { data_type, properties }
                }
                Object::String(s) => Value::Unicode(s.clone()),
            },
        }
    }

    /* TIM */
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
            // Slot::Object(h)     => match heap.get(*h) {
            //     Ok(Object::Array(a))       => format!("Array<{}>", a.element_type),
            //     Ok(Object::Class(c))       => format!("Class<{}>", c.name),
            //     Ok(Object::Function(f))    => format!("Function<{}>", f.name),
            //     Ok(Object::FunctionExt(f)) => format!("FunctionExt<{}; {}>", f.name, f.kind),
            //     Ok(Object::Instance(_))    => format!("Instance<{}>", "?"),
            //     Ok(Object::String(_))      => "String".to_string(),
            //     Err(_)                     => "Object<???>".to_string(),
            // },
            Slot::Object(_)     => "Object<???>".to_string(),
        }
    }
    /*******/
}

impl Display for Slot {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
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
            /* TIM */
            // Slot::Object(h) => unsafe {
            //     match h.get_unchecked() {
            //         Object::Array(a) => format!("array<{}>", a.element_type),
            //         Object::Class(c) => format!("class<{}>", c.name),
            //         Object::Function(f) => format!("function<{}>", f.name),
            //         Object::FunctionExt(f) => format!("function<{}; {}>", f.name, f.kind),
            //         Object::Instance(_) => format!("instance<{}>", "?"),
            //         Object::String(s) => format!("{:?}", s),
            //     }
            // },
            Slot::Object(_) => String::from("<Object>"),
            /*******/
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

#[derive(Debug)]
pub struct Stack {
    inner: Vec<Slot>,
    use_const: bool,
}

impl Default for Stack {
    fn default() -> Self {
        Self::new(STACK_MAX, false)
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
    ///
    ///
    ///
    pub fn new(
        size: usize,
        use_const: bool,
    ) -> Self {
        Self {
            inner: Vec::with_capacity(size),
            use_const,
        }
    }

    ///
    ///
    ///
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    ///
    ///
    ///
    #[inline]
    pub fn clear_from(
        &mut self,
        index: usize,
    ) {
        self.inner.truncate(index)
    }

    ///
    ///
    ///
    #[inline]
    pub fn get(
        &self,
        index: usize,
    ) -> &Slot {
        if let Some(slot) = self.inner.get(index) {
            slot
        } else {
            panic!("Expected value");
        }
    }

    /* TIM */
    /// **Edited: Changed to work with custom Heap.**
    ///
    /// Gets the slot at the given index as an Object.
    /// 
    /// **Arguments**
    ///  * `index`: The index of the slot to retrieve. Will throw errors if it is out-of-bounds.
    /// 
    /// **Returns**  
    /// Returns a Handle to the Object that we wanted to retrieve if index pointer to an Object, or a StackError otherwise.
    #[inline]
    pub fn get_object(
        &self,
        index: usize,
    ) -> Result<Handle, StackError> {
        if let Some(slot) = self.inner.get(index) {
            match slot {
                Slot::Object(h) => Ok(*h),
                _               => Err(StackError::UnexpectedType{ got: slot.data_type(), expected: "Object".to_string() }),
            }
        } else {
            Err(StackError::OutOfBoundsError{ i: index, capacity: self.inner.len() })
        }
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    pub fn copy_pop(
        &mut self,
        index: usize,
    ) {
        self.inner.swap_remove(index);
    }

    ///
    ///
    ///
    #[inline]
    pub fn copy_push(
        &mut self,
        index: usize,
    ) {
        self.push_unit();

        let length = self.inner.len();
        self.inner.copy_within(index..index + 1, length - 1);
    }

    ///
    ///
    ///
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    ///
    ///
    ///
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /* TIM */
    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top boolean on the stack without popping it.
    /// 
    /// **Returns**  
    /// The boolean's value if successfull, or a StackError otherwise.
    #[inline]
    pub fn peek_boolean(&mut self) -> Result<bool, StackError> {
        // Get the top value
        let top = self.inner.last();
        if let None = top { return Err(StackError::EmptyStackError{ what: "a boolean".to_string() }); }

        // Match the (hopefully) boolean value with the top one
        match top.unwrap() {
            Slot::False => Ok(false),
            Slot::True  => Ok(true),
            _           => Err(StackError::UnexpectedType{ expected: "Boolean".to_string(), got: top.unwrap().data_type().to_string() }),
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top slot of the stack.
    /// 
    /// **Returns**  
    /// The slot with its value if successfull, or a StackError otherwise. Note that it will never fail if you know the stack has at least one element, so you can safely call .unwrap() in that case.
    #[inline]
    pub fn pop(&mut self) -> Result<Slot, StackError> {
        // Try to get the last value
        let slot = self.inner.pop();
        if let None = slot { return Err(StackError::EmptyStackError{ what: "anything".to_string() }); }
        let slot = slot.unwrap();

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

    /* TIM */
    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top value of the stack as a boolean.
    /// 
    /// **Returns**  
    /// The boolean value if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_boolean(&mut self) -> Result<bool, StackError> {
        // Try to get the top value
        if let Some(slot) = self.inner.pop() {
            match slot {
                Slot::False => Ok(false),
                Slot::True  => Ok(true),
                _           => Err(StackError::UnexpectedType{ expected: "Boolean".to_string(), got: slot.data_type().to_string() }),
            }
        } else {
            Err(StackError::EmptyStackError{ what: "a boolean".to_string() })
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Changed to return a StackError instead of panicking.**
    ///
    /// Returns the top value of the stack as an integer.
    /// 
    /// **Returns**  
    /// The value of the integer if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_integer(&mut self) -> Result<i64, StackError> {
        if let Some(slot) = self.inner.pop() {
            match slot {
                Slot::ConstMinusTwo => Ok(-2),
                Slot::ConstMinusOne => Ok(-1),
                Slot::ConstZero     => Ok(0),
                Slot::ConstOne      => Ok(1),
                Slot::ConstTwo      => Ok(2),
                Slot::Integer(n)    => Ok(n),
                _                   => Err(StackError::UnexpectedType{ expected: "Integer".to_string(), got: slot.data_type().to_string() }),
            }
        } else {
            Err(StackError::EmptyStackError{ what: "an integer".to_string() })
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Changed to return a StackError instead of panicking + now working with the new custom Heap.**
    ///
    /// Returns the top value of the stack as an object.
    /// 
    /// **Returns**  
    /// A handle to the object if successfull, or a StackError otherwise.
    #[inline]
    pub fn pop_object(&mut self) -> Result<Handle, StackError> {
        if let Some(slot) = self.inner.pop() {
            match slot {
                Slot::Object(h) => Ok(h),
                _               => Err(StackError::UnexpectedType{ expected: "Object".to_string(), got: slot.data_type().to_string() }),
            }
        } else {
            Err(StackError::EmptyStackError{ what: "an object".to_string() })
        }
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    pub fn pop_real(&mut self) -> f64 {
        if let Some(slot) = self.inner.pop() {
            match slot {
                Slot::Real(r) => r,
                _ => panic!("Expecting a real."),
            }
        } else {
            panic!("Empty stack.");
        }
    }

    ///
    ///
    ///
    #[inline]
    pub fn pop_unit(&mut self) {
        if let Some(slot) = self.inner.pop() {
            match slot {
                Slot::Unit => (),
                _ => panic!("Expecting unit."),
            }
        } else {
            panic!("Empty stack.");
        }
    }

    ///
    ///
    ///
    #[inline]
    pub fn push(
        &mut self,
        slot: Slot,
    ) {
        self.inner.push(slot);
    }

    ///
    ///
    ///
    #[inline]
    pub fn push_boolean(
        &mut self,
        boolean: bool,
    ) {
        let boolean = match boolean {
            false => Slot::False,
            true => Slot::True,
        };

        self.inner.push(boolean);
    }

    ///
    ///
    ///
    #[inline]
    pub fn push_integer(
        &mut self,
        integer: i64,
    ) {
        let integer = if self.use_const {
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

        self.inner.push(integer);
    }

    /* TIM */
    /// **Edited: now working with the custom Heap.**
    ///
    /// Pushesa given Object handle onto the stack.
    /// 
    /// **Arguments**
    ///  * `object`: The handle to put on there.
    #[inline]
    pub fn push_object(
        &mut self,
        object: Handle,
    ) {
        self.inner.push(Slot::Object(object));
    }
    /*******/

    ///
    ///
    ///
    #[inline]
    pub fn push_real(
        &mut self,
        real: f64,
    ) {
        self.inner.push(Slot::Real(real));
    }

    ///
    ///
    ///
    #[inline]
    pub fn push_unit(&mut self) {
        self.inner.push(Slot::Unit);
    }

    ///
    ///
    ///
    #[inline]
    pub fn try_pop(&mut self) -> Option<Slot> {
        self.inner.pop()
    }

    ///
    ///
    ///
    #[inline]
    pub fn try_push(
        &mut self,
        slot: Option<Slot>,
    ) {
        if let Some(slot) = slot {
            self.inner.push(slot)
        }
    }
}

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
