use crate::{objects::Object, stack::Slot};
use crate::heap::{Handle, Heap, HeapError};


/* TIM */
/// Enum that collects the errors for all CallFrame-related issues
#[derive(Debug)]
pub enum CallFrameError {
    /// Error for when the internal instruction pointer (IP) is out-of-bounds
    IPOutOfBounds{ ip: usize, max: usize },
    /// Error for when a constant index is out-of-bounds
    ConstOutOfBounds{ index: usize, max: usize },

    /// Error for when we want to resolve a function object to the heap but we couldn't
    IllegalHandleError{ handle: Handle, err: HeapError },
    /// Error for when the handle does not point to a function
    IllegalFunctionError{ handle: Handle, target: String },
}

impl std::fmt::Display for CallFrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallFrameError::IPOutOfBounds{ ip, max }       => write!(f, "Instruction pointer is out-of-bounds for CallFrame ({} >= {})", ip, max),
            CallFrameError::ConstOutOfBounds{ index, max } => write!(f, "Constant index {} is out-of-bounds for CallFrame with {} constants", index, max),

            CallFrameError::IllegalHandleError{ handle, err }      => write!(f, "Function handle '{}' in CallFrame is illegal: {}", handle, err),
            CallFrameError::IllegalFunctionError{ handle, target } => write!(f, "Function handle '{}' does not point to a Function but to a {} instead", handle, target),
        }
    }
}

impl std::error::Error for CallFrameError {}
/*******/


///
///
///
#[derive(Copy, Clone, Debug)]
pub struct CallFrame {
    /* TIM */
    // pub function: Handle<Object>,
    pub function: Handle,
    /*******/
    pub ip: usize,
    pub stack_offset: usize,
}

impl CallFrame {
    /* TIM */
    /// Constructor for the CallFrame.
    /// 
    /// **Arguments**
    ///  * `function`: The function that owns this CallFrame.
    ///  * `stack_offset`: This frame's stack offset in the main stack.
    pub fn new(
        function: Handle,
        stack_offset: usize,
    ) -> Self {
        Self {
            function,
            ip: 0,
            stack_offset,
        }
    }

    /* TIM */
    /// **Edited: Changed return option to return a CallFrameError on failure instead of None. Also changed to work with the custom Heap.**
    ///
    /// Returns the next byte in the internal function's code.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap which we use to dereference the internal Function handle.
    /// 
    /// **Returns**  
    /// A reference to the next byte on success, or a CallFrameError upon failure.
    pub fn read_u8<'a>(&mut self, heap: &'a Heap<Object>) -> Result<&'a u8, CallFrameError> {
        // Get the function from the internal handle
        let function = match heap.get(self.function) {
            Ok(Object::Function(function)) => function,
            Ok(object)  => { return Err(CallFrameError::IllegalFunctionError{ handle: self.function, target: object.data_type() }); },
            Err(reason) => { return Err(CallFrameError::IllegalHandleError{ handle: self.function, err: reason }); }
        };

        // Get the next byte according to the instruction pointer
        let byte = function.chunk.code.get(self.ip);
        if let None = byte { return Err(CallFrameError::IPOutOfBounds{ ip: self.ip, max: function.chunk.code.len() }); }

        // Increment the instruction pointer and return
        self.ip += 1;
        Ok(byte.unwrap())
    }
    /*******/

    /* TIM */
    /// **Edited: Changed to return a CallFrameError instead of panicking. Also changed to work with the custom Heap.**
    /// 
    /// Reads the next two bytes of the function's code as a 16-bit unsigned integer.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap which we use to dereference the internal Function handle.
    /// 
    /// **Returns**  
    /// The 16-bit unsigned integer on success, or a CallFrameError upon failure.
    pub fn read_u16(&mut self, heap: &Heap<Object>) -> Result<u16, CallFrameError> {
        // Get the function from the internal handle
        let function = match heap.get(self.function) {
            Ok(Object::Function(function)) => function,
            Ok(object)  => { return Err(CallFrameError::IllegalFunctionError{ handle: self.function, target: object.data_type() }); },
            Err(reason) => { return Err(CallFrameError::IllegalHandleError{ handle: self.function, err: reason }); }
        };

        // Read the first byte
        let byte1 = function.chunk.code.get(self.ip);
        if let None = byte1 { return Err(CallFrameError::IPOutOfBounds{ ip: self.ip, max: function.chunk.code.len() }); }
        let byte1 = *byte1.unwrap() as u16;
        self.ip += 1;

        let byte2 = function.chunk.code.get(self.ip);
        if let None = byte2 { return Err(CallFrameError::IPOutOfBounds{ ip: self.ip, max: function.chunk.code.len() }); }
        let byte2 = *byte2.unwrap() as u16;
        self.ip += 1;

        // Return the result
        Ok((byte1 << 8) | byte2)
    }
    /*******/

    /* TIM */
    /// **Edited: Changed return option to return a CallFrameError on failure instead of None. Also changed to work with the custom Heap.**
    ///
    /// Returns the constant on the function's list of constants according to the next byte on the function's code.
    /// 
    /// **Arguments**
    ///  * `heap`: The Heap which we use to dereference the internal Function handle.
    /// 
    /// **Returns**  
    /// A reference to slot containing the value on success, or a CallFrameError upon failure.
    pub fn read_constant<'a>(&mut self, heap: &'a Heap<Object>) -> Result<&'a Slot, CallFrameError> {
        // Get the function from the internal handle
        let function = match heap.get(self.function) {
            Ok(Object::Function(function)) => function,
            Ok(object)  => { return Err(CallFrameError::IllegalFunctionError{ handle: self.function, target: object.data_type() }); },
            Err(reason) => { return Err(CallFrameError::IllegalHandleError{ handle: self.function, err: reason }); }
        };

        // Get the next byte as the index
        let index = function.chunk.code.get(self.ip);
        if let None = index { return Err(CallFrameError::IPOutOfBounds{ ip: self.ip, max: function.chunk.code.len() }); }
        let index = *(index.unwrap()) as usize;

        // Try to get the constant
        let constant = function.chunk.constants.get(index);
        if let None = constant { return Err(CallFrameError::ConstOutOfBounds{ index: index, max: function.chunk.constants.len() }); }

        // Update the instruction pointer and return!
        self.ip += 1;
        Ok(constant.unwrap())
    }
    /*******/
}
