/* HEAP.rs
 *   by Lut99
 *
 * Created:
 *   31 Jan 2022, 09:57:30
 * Last edited:
 *   09 Mar 2022, 14:45:43
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Custom heap implementation that is completely self-contained, and can
 *   thus be moved around without having to worry that pointers will be
 *   invalidated.
**/

use std::error::Error;
use std::fmt;
use std::fmt::Display;


/***** CONSTANTS *****/
/// Default recommended heap size to start with
const DEFAULT_HEAP_SIZE: usize = 512;





/***** ERRORS *****/
/// Enum that is a collection of all errors related to the Heap type
#[derive(Debug, PartialEq)]
pub enum HeapError {
    /// We ran out of heap space
    OutOfMemoryError{ capacity: usize },
    /// The given handle was out-of-bounds for this heap
    IllegalHandleError{ handle: Handle, capacity: usize },
    /// The given handle points to a non-initialized value
    DanglingHandleError{ handle: Handle },
}

impl Display for HeapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeapError::OutOfMemoryError{ capacity }           => write!(f, "Could not allocate new object on heap: out of memory (capacity: {} objects)", capacity),
            HeapError::IllegalHandleError{ handle, capacity } => write!(f, "Encountered illegal handle {}: handle index is out-of-bounds ({} >= {})", handle, handle, capacity),
            HeapError::DanglingHandleError{ handle }          => write!(f, "Encountered dangling handle {}", handle),
        }
    }
}

impl Error for HeapError {}





/***** HEAP *****/
/// A Handle to an object for our custom heap implementation.
pub type Handle = usize;



/// Custom Heap implementation that can be used to allocate heap-side data for the VM.
/// 
/// **Generic types**
///  * `T`: The type of the objects on the Heap. Since this means every element is always the same, this considerably speeds up allocation times.
#[derive(Debug, Clone)]
pub struct Heap<T> {
    /// The storage for the T
    data    : Vec<Option<T>>,
    /// Keeps track of how many slots are in-use on the heap.
    size    : usize,
    /// Determines whether or not the heap automatically resizes itself.
    resizes : bool,
}

impl<T> Heap<T> {
    /// Constructor for the Heap
    /// 
    /// **Arguments**
    ///  * `capacity`: The maximum capacity of the heap. Note that re-allocating is expensive (but not impossible), so choose something that doesn't have to realocate that often.
    ///  * `resizes`: Whether or not the Heap object automatically resizes.
    #[inline]
    pub fn new(capacity: usize, resizes: bool) -> Heap<T> {
        Heap {
            data: (0..capacity).map(|_| None).collect(),
            size: 0,
            resizes
        }
    }



    /// Puts the given object T on the heap, returning a handle to it.
    /// 
    /// **Arguments**
    ///  * `obj`: The Object to put on the heap.
    /// 
    /// **Returns**  
    /// A handle to the object allocated on the stack. Will be valid even if the memory of the Heap has been moved around. If the allocation failed, returns a HeapError.
    pub fn alloc(&mut self, obj: T) -> Result<Handle, HeapError> {
        // Find the first free slot in the vector
        for i in 0..self.data.len() {
            if let None = self.data[i] {
                // Use this spot
                self.data[i] = Some(obj);
                self.size += 1;
                // Return the handle
                return Ok(i as Handle);
            }
        }

        // Otherwise, no more memory available; panic if we don't automatically resize
        if self.resizes {
            // Resize either to 1 if we are 0 in size or double the size, then recurse to get a new slot
            if self.data.len() == 0 { self.reserve(1); }
            else { self.reserve(2 * self.capacity()); }
            self.alloc(obj)
        } else {
            // Do not resize
            Err(HeapError::OutOfMemoryError{ capacity: self.data.len() })
        }
    }

    /// Removes the object behind the given handle from the Heap, freeing its slot.
    /// 
    /// **Arguments**
    ///  * `handle`: The Handle to the object to remove. If it's an invalid handle, throws an error.
    /// 
    /// **Returns**  
    /// Nothing on success, or a HeapError otherwise.
    pub fn free(&mut self, handle: Handle) -> Result<(), HeapError> {
        // Check if this handle is valid
        if handle as usize >= self.data.len() { return Err(HeapError::IllegalHandleError{ handle: handle, capacity: self.data.len() }); }
        if let None = self.data[handle as usize] { return Err(HeapError::DanglingHandleError{ handle: handle }); }

        // Replace the value with None
        self.data[handle as usize] = None;
        self.size -= 1;
        Ok(())
    }



    /// Returns an immuteable reference to the object behind the given handle.
    /// 
    /// **Arguments**
    ///  * `handle`: Handle to the object to retrieve.
    /// 
    /// **Returns**  
    /// A reference to the object on success, or a HeapError otherwise.
    pub fn get(&self, handle: Handle) -> Result<&T, HeapError> {
        // Check if this handle is valid
        if handle as usize >= self.data.len() { return Err(HeapError::IllegalHandleError{ handle: handle, capacity: self.data.len() }); }
        if let None = self.data[handle as usize] { return Err(HeapError::DanglingHandleError{ handle: handle }); }
        
        // Try to get the value
        match self.data[handle as usize] {
            Some(ref obj) => Ok(obj),
            None          => Err(HeapError::DanglingHandleError{ handle: handle }),
        }
    }

    /// Returns a muteable reference to the object behind the given handle.
    /// 
    /// **Arguments**
    ///  * `handle`: Handle to the object to retrieve.
    /// 
    /// **Returns**  
    /// A reference to the object on success, or a HeapError otherwise.
    pub fn get_mut(&mut self, handle: Handle) -> Result<&mut T, HeapError> {
        // Check if this handle is valid
        if handle as usize >= self.data.len() { return Err(HeapError::IllegalHandleError{ handle: handle, capacity: self.data.len() }); }

        // Try to get the value
        match self.data[handle as usize] {
            Some(ref mut obj) => Ok(obj),
            None              => Err(HeapError::DanglingHandleError{ handle: handle }),
        }
    }

    /// Returns the object behind the given handle, removing it from the Heap.
    /// 
    /// **Arguments**
    ///  * `handle`: Handle to the object to retrieve.
    /// 
    /// **Returns**  
    /// The object on success, or a HeapError otherwise.
    pub fn take(&mut self, handle: Handle) -> Result<T, HeapError> {
        // Check if this handle is valid
        if handle as usize >= self.data.len() { return Err(HeapError::IllegalHandleError{ handle: handle, capacity: self.data.len() }); }
        if let None = self.data[handle as usize] { return Err(HeapError::DanglingHandleError{ handle: handle }); }

        // Return the value
        self.size -= 1;
        Ok(self.data[handle as usize].take().unwrap())
    }



    /// Resizes the heap to the given size.
    /// 
    /// If the new size is less than the old size, any elements that won't fit are discarded. Any new elements are left uninitialized.
    /// 
    /// **Arguments**
    ///  * `new_capacity`: The new capacity to resize the Heap to.
    #[inline]
    pub fn reserve(&mut self, new_capacity: usize) {
        if self.size > new_capacity { self.size = new_capacity; }
        self.data.resize_with(new_capacity, || None);
    }

    /// Returns the current number of occupied slots on the Heap.
    #[inline]
    pub fn len(&self) -> usize { self.size }

    /// Returns the total capacity of the Heap.
    #[inline]
    pub fn capacity(&self) -> usize { self.data.len() }
}

impl<T> Default for Heap<T> {
    /// Default constructor for the Heap
    #[inline]
    fn default() -> Heap<T> { Heap::new(DEFAULT_HEAP_SIZE, true) }
}
