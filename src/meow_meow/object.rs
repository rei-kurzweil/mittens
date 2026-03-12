use std::collections::HashMap;

/// Runtime value representation for Meow Meow evaluation.
///
/// This is intentionally small for v1 (component expressions) and is expected to evolve
/// as the language grows (objects, instances, closures, host interop, etc.).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),

    /// Heap-allocated object (map / record / instance).
    Object(ObjectId),

    /// Symbolic identifier value (e.g. positional flags like `QUAD_2D`).
    ///
    /// Keeping this distinct from `String` helps preserve intent.
    Identifier(String),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ObjectId(u32);

impl ObjectId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    /// Simple string-keyed map.
    Map(HashMap<String, Value>),
}

#[derive(Debug, Default)]
pub struct Heap {
    objects: Vec<Object>,
}

impl Heap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&mut self, object: Object) -> ObjectId {
        let id = ObjectId(
            self.objects
                .len()
                .try_into()
                .expect("too many heap objects"),
        );
        self.objects.push(object);
        id
    }

    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(id.0 as usize)
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}
