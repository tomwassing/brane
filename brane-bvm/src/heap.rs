/* HEAP.rs
 *   by Lut99
 *
 * Created:
 *   31 Jan 2022, 09:57:30
 * Last edited:
 *   21 Mar 2022, 21:53:51
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
use std::fmt::{Display, Formatter, Result as FResult};
use std::sync::Arc;


/***** CONSTANTS *****/
/// Default recommended heap size to start with
const DEFAULT_HEAP_SIZE: usize = 512;





/***** ERRORS *****/
/// Enum that is a collection of all errors related to the Heap type
#[allow(clippy::enum_variant_names)]
#[derive(Debug, PartialEq)]
pub enum HeapError {
    /// We ran out of heap space
    OutOfMemoryError{ capacity: usize },
    /// The given handle was out-of-bounds for this heap
    IllegalHandleError{ handle: String, capacity: usize },
    /// The given handle points to a non-initialized value
    DanglingHandleError{ handle: String },
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





/***** HELPER ENUMS *****/
/// Simple enum defining some states for the garbage collection loop.
enum GarbageCollectorState<T> {
    /// We can still insert a new element
    Pending(Arc<T>),
    /// We should just remove old ones
    Remove,
}





/***** HEAP *****/
/// A Handle to an object for our custom heap implementation.  
/// Basically just a wrapper around an Arc.
#[derive(Debug)]
pub struct Handle<T> {
    /// Reference to the object we're handling
    object: Arc<T>,
}

impl<T> Handle<T> {
    /// Returns an immuteable reference to the object behind the Handle.
    pub fn get(&self) -> &T {
        self.object.as_ref()
    }
}

impl<T> Clone for Handle<T> {
    #[inline]
    fn clone(&self) -> Self {
        Handle{ object: self.object.clone() }
    }
}

impl<T> PartialEq for Handle<T> {
    #[inline]
    fn eq(&self, other: &Handle<T>) -> bool {
        Arc::ptr_eq(&self.object, &other.object)
    }
}

impl<T> Display for Handle<T>
where
    T: Display
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "Handle<{}>", self.object)
    }
}



/// Custom Heap implementation that can be used to allocate heap-side data for the VM.
/// 
/// **Generic types**
///  * `T`: The type of the objects on the Heap. Since this means every element is always the same, this considerably speeds up allocation times.
#[derive(Debug)]
pub struct Heap<T> {
    /// The storage for the T.
    data     : Vec<Arc<T>>,
    /// Determines the maximum heap size
    max_size : usize,
}

impl<T> Heap<T> {
    /// Constructor for the Heap
    /// 
    /// **Arguments**
    ///  * `max_size`: The maximum size the Heap can grow. Use something ridiculously high to rely on memory limits instead.
    #[inline]
    pub fn new(max_size: usize) -> Heap<T> {
        Heap {
            data     : Vec::with_capacity(max_size),
            max_size,
        }
    }



    /// Puts the given object T on the heap, returning a handle to it.
    /// 
    /// **Arguments**
    ///  * `obj`: The Object to put on the heap.
    /// 
    /// **Returns**  
    /// A handle to the object allocated on the stack. Will be valid even if the memory of the Heap has been moved around. If the allocation failed, returns a HeapError.
    pub fn alloc(&mut self, obj: T) -> Result<Handle<T>, HeapError> {
        // Create the new element & its handle
        let elem   = Arc::new(obj);
        let handle = Handle{ object: elem.clone() };

        // First: check if there are any free slots in the vector
        let mut state = GarbageCollectorState::Pending(elem);
        for i in 0..self.data.len() {
            // Add extra checking to make sure we don't go out-of-bounds after garbage collection
            if i >= self.data.len() { break; }
            
            // Check if we need to remove this element
            if Arc::strong_count(&self.data[i]) == 1 {
                // Match the state
                state = match state {
                    GarbageCollectorState::Pending(elem) => {
                        // Replace it with the element
                        self.data[i] = elem;
                        GarbageCollectorState::Remove
                    },
                    GarbageCollectorState::Remove => {
                        // Remove it
                        self.data.swap_remove(i);
                        GarbageCollectorState::Remove
                    },
                };
            }
        }
        
        // If it wasn't found, we have to append to the end of the vector
        if let GarbageCollectorState::Pending(elem) = state {
            // Make sure we have space
            if self.data.len() >= self.max_size {
                return Err(HeapError::OutOfMemoryError{ capacity: self.max_size });
            }

            // Add it
            self.data.push(elem);
        }

        // Done! Return the handle
        Ok(handle)
    }



    /// Returns the current number of occupied slots on the Heap.
    #[inline]
    pub fn len(&self) -> usize { self.data.len() }

    /// Returns the total capacity of the Heap.
    #[inline]
    pub fn capacity(&self) -> usize { self.max_size }
}

impl<T> Default for Heap<T> {
    /// Default constructor for the Heap
    #[inline]
    fn default() -> Heap<T> { Heap::new(DEFAULT_HEAP_SIZE) }
}
