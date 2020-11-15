use std::ffi::c_void;

use cranelift_codegen::{
    binemit::NullTrapSink,
    ir::{self, ExternalName},
    settings::{self, Configurable},
};
use cranelift_module::{default_libcall_names, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use cranelift_wasm::{translate_module, DefinedFuncIndex, FuncTranslator};
use log::{debug, trace, warn};

#[macro_use]
mod signature;
mod function;
mod module;
mod runtime;

use self::{
    function::FunctionEnv,
    module::ModuleEnv,
    runtime::{Externs, Memory},
};
pub use self::{
    runtime::{Loadable, VMContext},
    signature::{ExternRef, FuncRef, Function},
};

/// A global value imported into a WASM module
///
/// At the moment only constant values (integers) are supported
#[derive(Debug)]
pub enum GlobalValue {
    Const(u32),
}

/// A handle to the host environment, used by the compiler to resolve import
/// requests from the WASM modules
pub trait Environment {
    fn import_function(&mut self, module: &str, name: &str) -> Option<Function>;
    fn import_global(&mut self, module: &str, name: &str) -> Option<GlobalValue>;
}

/// Loads a module from a WAT text source: this will parse the module from
/// source, translate it to machine code and execute the `start` function
/// if there is one before returning the newly constructed VMContext
pub fn load_module<E: Environment>(environment: E, source: &str) -> VMContext<E> {
    // Parse the WAT source
    let source = match wat::parse_str(source) {
        Ok(source) => source,
        Err(err) => {
            warn!("could not load source: {}", err);
            panic!("{:?}", err)
        }
    };

    // Translate the module: this does NOT translate the function bodies yet,
    // it only load the general structure of the module into the `environment`
    let mut environment = ModuleEnv::new(environment);
    let state = translate_module(&source, &mut environment).unwrap();

    let ModuleEnv {
        env: environment,
        module: defs,

        memories,
        data_initializations,

        start_func,
        imported_functions,
        defined_functions,
    } = environment;

    // Initialize the JIT backend for the native ISA
    let mut flag_builder = settings::builder();
    flag_builder.set("enable_safepoints", "true").unwrap();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();

    let isa_builder = cranelift_native::builder().unwrap();
    let isa = isa_builder.finish(settings::Flags::new(flag_builder));

    let mut builder = SimpleJITBuilder::with_isa(isa, default_libcall_names());

    // Load all imported function pointers in the linker
    for (_, (name, ptr)) in &imported_functions {
        builder.symbol(name, *ptr);
    }

    // Create an empty Cranelift module
    let mut module: Module<SimpleJITBackend> = Module::new(builder);

    let mut list = Vec::new();
    let mut translator = FuncTranslator::new();

    // Insert all the functions (imported and defined) in the module
    for (func_index, sig_index) in &defs.functions {
        let signature = &defs.signatures[*sig_index];

        // Will be Some(_) if this is an imported function
        let imported_function =
            imported_functions.get(DefinedFuncIndex::from_u32(func_index.as_u32()));

        // Will be Some(_) if this is an defined function
        let defined_function = func_index
            .as_u32()
            .checked_sub(imported_functions.len() as u32)
            .and_then(|index| defined_functions.get(DefinedFuncIndex::from_u32(index)));

        // Declare the function by name (using a placeholder name for defined functions)
        // All functions must be declared in the same order as the original module (imports
        // then definitions) so the linker can map the various ExternalNames to the right symbols
        let name = match imported_function {
            Some((name, _)) => name.clone(),
            None => format!("func_{}", func_index.as_u32()),
        };

        let id = module
            .declare_function(
                &name,
                if defined_function.is_some() {
                    Linkage::Export
                } else {
                    Linkage::Import
                },
                &signature.clif,
            )
            .unwrap();

        // If this is a defined function, run the translator on the WASM body
        // and register the result ir::Function in the module as a definition
        // for the previously created FuncId
        if let Some(body) = defined_function {
            let mut context = module.make_context();
            context.func = ir::Function::with_name_signature(
                ExternalName::user(0, func_index.as_u32()),
                signature.clif.clone(),
            );

            translator
                .translate(
                    &state,
                    body.body_bytes,
                    body.body_offset,
                    &mut context.func,
                    &mut FunctionEnv { module: &defs },
                )
                .unwrap();

            debug!("{:?}", context.func);

            module
                .define_function(id, &mut context, &mut NullTrapSink::default())
                .unwrap();

            list.push(Some((id, signature.clone())));
        } else {
            list.push(None);
        }
    }

    // Finalize the module generation and emit the machine code
    module.finalize_definitions();

    // Fill the functions table with pointers to the emitted functions
    let functions: Vec<_> = list
        .into_iter()
        .map(|entry| {
            entry.map(|(id, signature)| Function {
                signature,
                pointer: module.get_finalized_function(id),
            })
        })
        .collect();

    trace!("functions {:?}", functions);

    // Initialize the linear memory with the static data defined in the module
    let mut memory = Vec::new();

    for (index, _) in memories {
        let init = &data_initializations[index];

        let init_len = init.data.len();
        let init_end = init.offset + init_len;
        if memory.len() < init_end {
            memory.resize(init_end, 0);
        }

        let memory = &mut memory[init.offset..];
        let memory = &mut memory[..init_len];
        memory.copy_from_slice(init.data);
    }

    // Create the VMContext object
    let mut context = VMContext {
        _handle: module.finish(),

        functions,

        memory: Memory::new(memory),
        externs: Externs::default(),

        environment,
    };

    type EntryFunc<E> = with_abi!(fn(*mut VMContext<E>));

    // Execute the `start` function if the module has one
    if let Some(index) = start_func {
        if let Some(func) = &context.functions[index.as_u32() as usize] {
            let func: EntryFunc<E> = func.get();
            debug!("Calling start function at {:?}", func as *const c_void);
            func(&mut context);
        }
    }

    context
}
