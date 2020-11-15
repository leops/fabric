#![feature(abi_thiscall)]
#![feature(const_fn)]
#![feature(const_fn_fn_ptr_basics)]

use log::warn;
use std::{
    ffi::{c_void, CStr},
    os::raw::{c_char, c_int},
    ptr::null_mut,
};

mod addon;
mod foreign;
mod logging;
mod manager;
mod module;

#[ctor::ctor]
fn __init_logs() {
    crate::logging::init_logger();
}

#[no_mangle]
pub extern "C" fn CreateInterface(name: *const c_char, return_code: *mut c_int) -> *mut c_void {
    let name = unsafe { CStr::from_ptr(name) };
    let name = name.to_string_lossy();

    match &*name {
        "ISERVERPLUGINCALLBACKS003" => {
            let return_code = unsafe { return_code.as_mut() };
            if let Some(return_code) = return_code {
                *return_code = 0;
            }

            unsafe { &mut crate::addon::INSTANCE as *mut _ as *mut c_void }
        }
        name => {
            warn!("Unknown interface {}", name);

            let return_code = unsafe { return_code.as_mut() };
            if let Some(return_code) = return_code {
                *return_code = 1;
            }

            null_mut()
        }
    }
}
