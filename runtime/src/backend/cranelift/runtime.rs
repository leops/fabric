use std::{
    any::Any,
    ffi::CStr,
    fmt::{self, Debug, Formatter},
};

use cranelift_module::Backend;
use cranelift_simplejit::SimpleJITBackend;

use super::signature::Function;
use crate::{ExternRef, FuncRef};

/// A compiled module. It holds the functions table, linear
/// memory, externs arena and host environment for the module,
// and an exclusive (mut) reference to it must be passed as an
// argument to all functions emitted from this
pub struct VMContext<E> {
    pub(crate) _handle: <SimpleJITBackend as Backend>::Product,
    pub(crate) functions: Vec<Option<Function>>,

    /// Linear memory instance associated with this module
    pub memory: Memory,

    /// Arena holding the managed externals for this instance
    pub externs: Externs,

    /// Handle to the host environment
    pub environment: E,
}

impl<E: Debug> Debug for VMContext<E> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("VMContext")
            .field("memory", &self.memory)
            .field("functions", &self.functions)
            .field("environment", &self.environment)
            .finish()
    }
}

impl<E> VMContext<E> {
    /// Get a function handle from a WASM function reference
    pub fn function(&self, index: FuncRef) -> Option<&Function> {
        self.functions
            .get(index.0 as usize)
            .and_then(Option::as_ref)
    }
}

/// WASM linear memory instance
#[derive(Debug)]
pub struct Memory(Vec<u8>);

impl Memory {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Memory(data)
    }

    /// Load a value from memory
    ///
    /// This is implemented with a separate Loadable trait so the turbofish syntax
    /// `memory.load::<T>(offset)` can be used with this method
    pub fn load<T: Loadable + ?Sized>(&self, offset: usize) -> Result<&T, T::Error> {
        T::load(&self.0, offset)
    }
}

pub trait Loadable {
    type Error;
    fn load(memory: &[u8], offset: usize) -> Result<&Self, Self::Error>;
}

impl Loadable for [u8] {
    type Error = ();

    fn load(memory: &[u8], offset: usize) -> Result<&[u8], Self::Error> {
        match memory.get(offset..) {
            Some(slice) => Ok(slice),
            None => Err(()),
        }
    }
}

impl Loadable for CStr {
    type Error = ();

    fn load(memory: &[u8], offset: usize) -> Result<&CStr, Self::Error> {
        let memory = <[u8]>::load(memory, offset)?;

        let end = match memory.iter().position(|byte| *byte == 0) {
            Some(end) => end,
            None => return Err(()),
        };

        match CStr::from_bytes_with_nul(&memory[..end]) {
            Ok(value) => Ok(value),
            Err(_) => Err(()),
        }
    }
}

/// Arena holding managed external objects for a given module
#[derive(Default)]
pub struct Externs(Vec<ExternSlot>);

pub(crate) struct ExternSlot {
    gen: u16,
    value: Option<Box<dyn Any>>,
}

impl Externs {
    /// Moves `value` to the externs table, returning the allocated slot index as an ExternRef
    pub fn create_extern<T: Any>(&mut self, value: T) -> ExternRef {
        let value = Box::new(value);

        for (index, slot) in self.0.iter_mut().enumerate() {
            if slot.value.is_none() {
                slot.gen += 1;
                slot.value = Some(value);
                return ExternRef::from_index_gen(index as u32, slot.gen);
            }
        }

        let index = self.0.len();

        self.0.push(ExternSlot {
            gen: 0,
            value: Some(value),
        });

        ExternRef::from_index_gen(index as u32, 0)
    }

    /// Get a reference to the object corresponding to a given ExternRef
    pub fn get_extern<T: Any>(&self, index: ExternRef) -> &T {
        let (index, gen) = index.index_gen();
        let slot = &self.0[index as usize];

        assert_eq!(slot.gen, gen);

        slot.value.as_ref().unwrap().downcast_ref().unwrap()
    }

    /// Get a mutable reference to the object corresponding to a given ExternRef
    pub fn get_extern_mut<T: Any>(&mut self, index: ExternRef) -> &mut T {
        let (index, gen) = index.index_gen();
        let slot = &mut self.0[index as usize];

        assert_eq!(slot.gen, gen);

        slot.value.as_mut().unwrap().downcast_mut().unwrap()
    }

    /// Take ownership of the object corresponding to a given ExternRef,
    // removing it from the arena
    pub fn take_extern<T: Any>(&mut self, index: ExternRef) -> T {
        let (index, gen) = index.index_gen();
        let slot = &mut self.0[index as usize];

        assert_eq!(slot.gen, gen);

        *slot.value.take().unwrap().downcast().unwrap()
    }
}
