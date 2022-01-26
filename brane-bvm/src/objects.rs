use crate::bytecode::{ClassMut, FunctionMut};
use crate::{bytecode::Chunk, stack::Slot};
use broom::prelude::*;
use fnv::FnvHashMap;
use specifications::common::FunctionExt;
/* TIM */
use specifications::common::Typed;
/*******/


/* TIM */
/// Enum for Object-related errors
#[derive(Debug)]
pub enum ObjectError {
    /// Error for when the type of an Array could not be established
    ArrayError{ array: Array, type1: String, type2: String },
}

impl std::fmt::Display for ObjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectError::ArrayError{ array, type1, type2 } =>
                write!(f, "Could not resolve type of Array '{:?}': conflicting types '{}' and '{}'", array, type1, type2)
        }
    }
}

impl std::error::Error for ObjectError {}

/*******/


#[derive(Debug)]
pub enum Object {
    Array(Array),
    Class(Class),
    Function(Function),
    FunctionExt(FunctionExt),
    Instance(Instance),
    String(String),
}

impl Object {
    #[inline]
    pub fn as_class(&self) -> Option<&Class> {
        if let Object::Class(class) = self {
            Some(class)
        } else {
            None
        }
    }

    #[inline]
    pub fn as_function(&self) -> Option<&Function> {
        if let Object::Function(function) = self {
            Some(function)
        } else {
            None
        }
    }

    #[inline]
    pub fn as_string(&self) -> Option<&String> {
        if let Object::String(string) = self {
            Some(string)
        } else {
            None
        }
    }
}

/* TIM */
impl std::fmt::Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl Typed for Object {
    /// Returns the type of the object, as a string.
    fn data_type(&self) -> String {
        match self {
            Object::Array(a)       => format!("Array<{}>", a.element_type),
            Object::Class(c)       => format!("Class<{}>", c.name),
            Object::Function(f)    => format!("Function<{}>", f.name),
            Object::FunctionExt(f) => format!("FunctionExt<{}; {}>", f.name, f.kind),
            Object::Instance(_)    => format!("Instance<{}>", "???"),
            Object::String(_)      => "String".to_string(),
        }
    }
}
/*******/

// Tell the garbage collector how to explore a graph of this object
impl Trace<Self> for Object {
    fn trace(
        &self,
        tracer: &mut Tracer<Self>,
    ) {
        match self {
            Object::Array(a) => a.trace(tracer),
            Object::Class(c) => c.trace(tracer),
            Object::Function(f) => f.trace(tracer),
            Object::FunctionExt(_f) => {}
            Object::Instance(i) => i.trace(tracer),
            Object::String(_) => {}
        }
    }
}
#[derive(Debug, Clone)]
pub struct Array {
    pub element_type: String,
    pub elements: Vec<Slot>,
}

impl Array {
    pub fn new(elements: Vec<Slot>) -> Self {
        let element_type = if elements.is_empty() {
            String::from("unit")
        } else {
            String::from("???")
        };

        Self { element_type, elements }
    }

    /* TIM */
    /// Function that resolves the array's type in case it's '???'
    /// 
    /// **Arguments**
    ///  * `heap`: The heap to collect the values from.
    /// 
    /// **Returns**  
    /// Nothing if the type resolving was successful, or an Err with the reason otherwise.
    pub fn resolve_type(&mut self, heap: &Heap<Object>) -> Result<(), ObjectError> {
        // Skip if it doesn't need resolving
        if !self.element_type.eq("???") { return Ok(()); }

        // Go through the elements to establish a subtype
        let mut subtype = String::new();
        for elem in &self.elements {
            let elemval = elem.into_value(heap);
            let elemtype = elemval.data_type();
            if subtype.len() == 0 { subtype = String::from(elemtype); }
            else if !elemtype.eq(&subtype) {
                return Err(ObjectError::ArrayError{
                    array: self.clone(),
                    type1: subtype,
                    type2: String::from(elemtype)
                });
            }
        }

        // Set this subtype as the array's one but with '[]' and done
        self.element_type = format!("{}[]", subtype);
        Ok(())
    }
    /*******/
}

/* TIM */
impl std::fmt::Display for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Err(reason) = write!(f, "[") { return Err(reason); }
        for elem in &self.elements {
            if let Err(reason) = write!(f, "{}", elem) { return Err(reason); }
        }
        Ok(())
    }
}
/*******/

impl Trace<Object> for Array {
    fn trace(
        &self,
        _tracer: &mut Tracer<Object>,
    ) {
    }
}

#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub methods: FnvHashMap<String, Slot>,
}

impl Class {
    ///
    ///
    ///
    pub fn unfreeze(
        self,
        heap: &Heap<Object>,
    ) -> ClassMut {
        let methods = self
            .methods
            .into_iter()
            .map(|(k, v)| {
                let function = v.as_object().unwrap();
                let function = heap.get(function).unwrap();
                let function = function.as_function().unwrap();
                let function = function.clone().unfreeze(heap);

                (k, function)
            })
            .collect();

        ClassMut {
            name: self.name,
            properties: Default::default(),
            methods,
        }
    }
}

/* TIM */
impl std::fmt::Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
/*******/

impl Trace<Object> for Class {
    fn trace(
        &self,
        _tracer: &mut Tracer<Object>,
    ) {
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub arity: u8,
    pub chunk: Chunk,
    pub name: String,
}

impl Function {
    ///
    ///
    ///
    pub fn new(
        name: String,
        arity: u8,
        chunk: Chunk,
    ) -> Self {
        Self { arity, chunk, name }
    }

    ///
    ///
    ///
    pub fn unfreeze(
        self,
        heap: &Heap<Object>,
    ) -> FunctionMut {
        FunctionMut::new(self.name, self.arity, self.chunk.unfreeze(heap))
    }
}

/* TIM */
impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}()", self.name)
    }
}
/*******/

impl Trace<Object> for Function {
    fn trace(
        &self,
        _tracer: &mut Tracer<Object>,
    ) {
    }
}

#[derive(Debug)]
pub struct Instance {
    pub class: Handle<Object>,
    pub properties: FnvHashMap<String, Slot>,
}

impl Instance {
    ///
    ///
    ///
    pub fn new(
        class: Handle<Object>,
        properties: FnvHashMap<String, Slot>,
    ) -> Self {
        Self { class, properties }
    }
}

/* TIM */
impl std::fmt::Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<instance>")
    }
}
/*******/

impl Trace<Object> for Instance {
    fn trace(
        &self,
        tracer: &mut Tracer<Object>,
    ) {
        self.class.trace(tracer);
    }
}
