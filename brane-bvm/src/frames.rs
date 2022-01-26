use crate::{objects::Object, stack::Slot};
use broom::Handle;


/* TIM */
/// Enum that collects the errors for all CallFrame-related issues
#[derive(Debug)]
pub enum CallFrameError {
    /// Error for when the internal instruction pointer (IP) is out-of-bounds
    IPOutOfBounds{ ip: usize, max: usize },
    /// Error for when a constant index is out-of-bounds
    ConstOutOfBounds{ index: usize, max: usize },

    /// Error for when we could not get the function behind a handle
    DanglingFunctionError,
}

impl std::fmt::Display for CallFrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallFrameError::IPOutOfBounds{ ip, max }       => write!(f, "Instruction pointer is out-of-bounds for CallFrame ({} >= {})", ip, max),
            CallFrameError::ConstOutOfBounds{ index, max } => write!(f, "Constant index {} is out-of-bounds for CallFrame with {} constants", index, max),

            CallFrameError::DanglingFunctionError => write!(f, "Function handle in CallFrame is dangling"),
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
    pub function: Handle<Object>,
    pub ip: usize,
    pub stack_offset: usize,
}

impl CallFrame {
    ///
    ///
    ///
    pub fn new(
        function: Handle<Object>,
        stack_offset: usize,
    ) -> Self {
        Self {
            function,
            ip: 0,
            stack_offset,
        }
    }

    /* TIM */
    /// **Edited: Changed return option to return a CallFrameError on failure instead of None.**
    ///
    /// Returns the next byte in the internal function's code.
    /// 
    /// **Returns**  
    /// A reference to the next byte on success, or a CallFrameError upon failure.
    pub fn read_u8(&mut self) -> Result<&u8, CallFrameError> {
        unsafe {
            // Get the function from the internal handle
            let function = self.function.get_unchecked().as_function();
            if let None = function { return Err(CallFrameError::DanglingFunctionError); }
            let function = function.unwrap();

            // Get the next byte according to the instruction pointer
            let byte = function.chunk.code.get(self.ip);
            if let None = byte { return Err(CallFrameError::IPOutOfBounds{ ip: self.ip, max: function.chunk.code.len() }); }

            // Increment the instruction pointer and return
            self.ip += 1;
            Ok(byte.unwrap())
        }
    }
    /*******/

    /* TIM */
    /// **Edited: Changed to return a CallFrameError instead of panicking.**
    /// 
    /// Reads the next two bytes of the function's code as a 16-bit unsigned integer.
    /// 
    /// **Returns**  
    /// The 16-bit unsigned integer on success, or a CallFrameError upon failure.
    pub fn read_u16(&mut self) -> Result<u16, CallFrameError> {
        unsafe {
            // Get the function from the internal handle
            let function = self.function.get_unchecked().as_function();
            if let None = function { return Err(CallFrameError::DanglingFunctionError); }
            let function = function.unwrap();

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
    }
    /*******/

    /* TIM */
    /// **Edited: Changed return option to return a CallFrameError on failure instead of None.**
    ///
    /// Returns the constant on the function's list of constants according to the next byte on the function's code.
    /// 
    /// **Returns**  
    /// A reference to slot containing the value on success, or a CallFrameError upon failure.
    pub fn read_constant(&mut self) -> Result<&Slot, CallFrameError> {
        unsafe {
            // Get the function from the internal handle
            let function = self.function.get_unchecked().as_function();
            if let None = function { return Err(CallFrameError::DanglingFunctionError); }
            let function = function.unwrap();

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
    }
    /*******/
}
