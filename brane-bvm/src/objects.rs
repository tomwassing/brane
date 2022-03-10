use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};

use fnv::FnvHashMap;
use specifications::common::FunctionExt;

use crate::bytecode::{ClassMut, FunctionMut};
use crate::{bytecode::Chunk, stack::Slot};
use crate::heap::Handle;


/***** ERRORS *****/
/// Enum for Object-related errors
#[derive(Debug, PartialEq)]
pub enum ObjectError {
    /// Error for when the type of an Array could not be established
    ArrayError{ array: Vec<Slot>, type1: String, type2: String },
}

impl Display for ObjectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            ObjectError::ArrayError{ array, type1, type2 } =>
                write!(f, "Could not resolve type of Array '{:?}': conflicting types '{}' and '{}'", array, type1, type2)
        }
    }
}

impl Error for ObjectError {}





/***** LIBRARY STRUCTS *****/
/// **Edited: working with errors, new Heap + docstring.**
/// 
/// Implements a Heap-side Object in the brane-vm.
#[derive(Debug)]
pub enum Object {
    /// An array.
    Array(Array),
    /// A class.
    Class(Class),
    /// A (local) function.
    Function(Function),
    /// A function that should be executed as a job.
    FunctionExt(FunctionExt),
    /// An instance of a class.
    Instance(Instance),
    /// A string.
    String(String),
}

impl Object {
    /// Tries to cast the Object to a Class.
    /// 
    /// **Returns**  
    /// A reference to the Class on success, or None otherwise.
    #[inline]
    pub fn as_class(&self) -> Option<&Class> {
        if let Object::Class(class) = self {
            Some(class)
        } else {
            None
        }
    }

    /// Tries to cast the Object to a Function.
    /// 
    /// **Returns**  
    /// A reference to the Function on success, or None otherwise.
    #[inline]
    pub fn as_function(&self) -> Option<&Function> {
        if let Object::Function(function) = self {
            Some(function)
        } else {
            None
        }
    }

    /// Tries to cast the Object to a String.
    /// 
    /// **Returns**  
    /// A reference to the String on success, or None otherwise.
    #[inline]
    pub fn as_string(&self) -> Option<&String> {
        if let Object::String(string) = self {
            Some(string)
        } else {
            None
        }
    }



    /// Returns the type of the object as a string.
    pub fn data_type(&self) -> String {
        match self {
            Object::Array(a)       => format!("Array<{}>", a.element_type),
            Object::Class(c)       => format!("Class<{}>", c.name),
            Object::Function(f)    => format!("Function<{}>", f.name),
            Object::FunctionExt(f) => format!("FunctionExt<{}; {}>", f.name, f.kind),
            Object::Instance(i)    => format!("Instance<{}>", i.class.get().as_class().expect("Instance parent is not a Class").name),
            Object::String(_)      => "String".to_string(),
        }
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            Object::Array(array) => write!(f, "{}", array),
            Object::Class(class) => write!(f, "{}", class),
            Object::Function(func) => write!(f, "{}", func),
            Object::FunctionExt(func_ext) => write!(f, "{}", func_ext),
            Object::Instance(instance) => write!(f, "{}", instance),
            Object::String(string) => write!(f, "{}", string),
        }
    }
}



/// Represents a heap-allocated Array object.
#[derive(Debug, Clone)]
pub struct Array {
    /// The type of this Array
    pub element_type: String,
    /// The elements, as Stack slots.  
    /// Note that these elements do not actually live on the stack, but rather to optimize taking values from the stack.
    pub elements: Vec<Slot>,
}

impl Array {
    /// Constructor for the Array.
    /// 
    /// **Arguments**
    ///  * `elements`: The list of elements that are in this Array. Will be used to deduce the Array's type from.
    /// 
    /// **Returns**  
    /// The new Array if we could resolve the type, or an ObjectError otherwise.
    pub fn new(elements: Vec<Slot>) -> Result<Self, ObjectError> {
        // Try to deduce the type from the elements
        let element_type = {
            // Iterate through the slots to find the subtype
            let mut subtype = String::from("unit");
            for elem in &elements {
                let elemval = elem.clone().into_value();
                let elemtype = elemval.data_type();
                if subtype.len() == 0 { subtype = String::from(elemtype); }
                else if !elemtype.eq(&subtype) {
                    return Err(ObjectError::ArrayError{
                        array: elements,
                        type1: subtype,
                        type2: String::from(elemtype)
                    });
                }
            }
            subtype
        };

        // Return an Array of that type
        Ok(Array {
            element_type,
            elements
        })
    }
}

/* TIM */
impl Display for Array {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "[")?;
        let mut first = true;
        for elem in &self.elements {
            if first { first = false; }
            else { write!(f, ",")?; }
            write!(f, "{}", elem)?;
        }
        write!(f, "]")
    }
}



/// Defines a custom type in the brane-vm.
#[derive(Clone, Debug)]
pub struct Class {
    /// The typename
    pub name: String,
    /// A list of methods supported by this type.  
    /// The slot is the actual Function object to use.
    pub methods: FnvHashMap<String, Slot>,
}

impl Class {
    /// **Edited: now working with the new Heap class.**
    /// 
    /// Unfreezes the Class (consuming it).  
    /// This means the data from the Class that is on the Heap (i.e., its methods) will be taken from the heap and readied for use.
    /// 
    /// **Returns**  
    /// A ClassMut, with unset properties and unfrozen functions.
    pub fn unfreeze(self) -> ClassMut {
        // Unfreeze the methods
        let methods = self
            .methods
            .into_iter()
            .map(|(k, v)| {
                // Unpack the slot as a handle
                let function = v.as_object().expect(&format!("Method {} is not an object", k));
                // Get the handle as a function
                let function = function.get();
                let function = function.as_function().expect(&format!("Method {} is not an object", k));
                // Unfreeze the function too
                (k, function.clone().unfreeze())
            })
            .collect();

        // Bundle the unfrozen methods in a ClassMut
        ClassMut {
            name: self.name,
            properties: Default::default(),
            methods,
        }
    }
}

impl Display for Class {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "<{}>", self.name)
    }
}



/// Defines a (local) Function in the brane-vm.
#[derive(Debug, Clone)]
pub struct Function {
    /// The arity (=number of arguments) of this function.
    pub arity: u8,
    /// The Chunk containing the bytecode that this Function should run.
    pub chunk: Chunk,
    /// The name of the function.
    pub name: String,
}

impl Function {
    /// Constructor for the Function.
    /// 
    /// **Arguments**
    ///  * `name`: The name of the Function.
    ///  * `arity`: The arity (=number of arguments) for this Function.
    ///  * `chunk`: The bytecode to execute for this Function.
    #[inline]
    pub fn new(
        name: String,
        arity: u8,
        chunk: Chunk,
    ) -> Self {
        Self { arity, chunk, name }
    }



    /// **Edited: now working with the new Heap class (actually taking into account chunks do).**
    /// 
    /// Unfreezes the Function (consuming it).  
    /// This means the data from the Function that is on the Heap (i.e., its bytecode) will be taken off the heap and readied for execution.
    /// 
    /// **Returns**  
    /// A FunctionMut, with unfrozen bytecode.
    #[inline]
    pub fn unfreeze(self) -> FunctionMut {
        FunctionMut::new(self.name, self.arity, self.chunk.unfreeze())
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "{}()", self.name)
    }
}



/// Defines an instantiated Class in the brane-bvm.
#[derive(Debug)]
pub struct Instance {
    /// The parent class that this Instance is an instance of.
    pub class: Handle<Object>,
    /// The list of actual property values that make this an instance.
    pub properties: FnvHashMap<String, Slot>,
}

impl Instance {
    /// **Edited: now works with custom Heap.**
    ///
    /// Constructor for the Instance.
    /// 
    /// **Arguments**  
    ///  * `class`: The class that forms the base of this Instance.
    ///  * `properties`: The list of properties for this Instance.
    #[inline]
    pub fn new(
        class: Handle<Object>,
        properties: FnvHashMap<String, Slot>,
    ) -> Self {
        Self { class, properties }
    }
}

impl Display for Instance {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "{}", self.class.get().as_class().expect("Instance parent is not a class").name)
    }
}
