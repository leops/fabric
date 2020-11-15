#![recursion_limit = "2048"]

mod backend;

pub use crate::backend::cranelift::{
    load_module, Environment, ExternRef, FuncRef, Function, GlobalValue, Loadable, VMContext,
};
