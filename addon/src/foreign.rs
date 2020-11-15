use std::{
    ffi::{c_void, CStr},
    marker::PhantomData,
    os::raw::{c_char, c_int},
};

use log::debug;

#[repr(transparent)]
pub(crate) struct Foreign<T: ?Sized>(pub(crate) *mut c_void, PhantomData<*mut T>);

impl<T: ?Sized> Foreign<T> {
    pub(crate) fn with(ptr: *mut c_void) -> Self {
        Foreign(ptr, PhantomData)
    }
}

pub(crate) type CreateInterfaceFn = extern "C" fn(*const c_char, *mut c_int) -> *mut c_void;

pub(crate) fn create_interface<T: ?Sized>(
    factory: CreateInterfaceFn,
    name: &CStr,
) -> Option<Foreign<T>> {
    let mut is_ok = 0;
    let pointer = factory(name.as_ptr(), &mut is_ok);

    if is_ok == 0 {
        debug!("create_interface {} {:?}", name.to_string_lossy(), pointer);
        Some(Foreign(pointer, PhantomData))
    } else {
        None
    }
}
