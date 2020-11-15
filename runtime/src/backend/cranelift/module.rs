use cranelift_codegen::{
    ir::{self},
    isa::TargetFrontendConfig,
};
use cranelift_entity::{PrimaryMap, SecondaryMap};
use cranelift_wasm::{
    DataIndex, DefinedFuncIndex, ElemIndex, FuncIndex, Global, GlobalIndex, Memory, MemoryIndex,
    ModuleEnvironment, ModuleTranslationState, SignatureIndex, Table, TableIndex,
    TargetEnvironment, WasmError, WasmFuncType, WasmResult, WasmType,
};
use log::trace;

use super::{
    signature::{Signature, CALL_CONV, POINTER_WIDTH},
    Environment, GlobalValue,
};

#[derive(Debug)]
pub(crate) struct ModuleEnv<'data, E> {
    pub(crate) env: E,
    pub(crate) module: ModuleDefs,
    pub(crate) start_func: Option<FuncIndex>,

    pub(crate) memories: PrimaryMap<MemoryIndex, Memory>,
    pub(crate) data_initializations: SecondaryMap<MemoryIndex, DataInitialization<'data>>,

    pub(crate) imported_functions: PrimaryMap<DefinedFuncIndex, (String, *const u8)>,
    pub(crate) defined_functions: PrimaryMap<DefinedFuncIndex, FunctionBody<'data>>,
}

#[derive(Debug, Default)]
pub(crate) struct ModuleDefs {
    pub(crate) globals: PrimaryMap<GlobalIndex, GlobalValue>,
    pub(crate) functions: PrimaryMap<FuncIndex, SignatureIndex>,
    pub(crate) signatures: PrimaryMap<SignatureIndex, Signature>,
}

#[derive(Debug)]
pub(crate) struct FunctionBody<'data> {
    pub(crate) body_bytes: &'data [u8],
    pub(crate) body_offset: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DataInitialization<'data> {
    base: Option<GlobalIndex>,
    pub(crate) offset: usize,
    pub(crate) data: &'data [u8],
}

impl<'data, E: Environment> ModuleEnv<'data, E> {
    pub(crate) fn new(env: E) -> Self {
        ModuleEnv {
            env,
            module: Default::default(),
            start_func: Default::default(),

            memories: Default::default(),
            data_initializations: Default::default(),

            imported_functions: Default::default(),
            defined_functions: Default::default(),
        }
    }
}

impl<'data, E> TargetEnvironment for ModuleEnv<'data, E> {
    fn target_config(&self) -> TargetFrontendConfig {
        TargetFrontendConfig {
            default_call_conv: CALL_CONV,
            pointer_width: POINTER_WIDTH,
        }
    }
}

impl<'data, E: Environment> ModuleEnvironment<'data> for ModuleEnv<'data, E> {
    fn declare_signature(&mut self, wasm: WasmFuncType, clif: ir::Signature) -> WasmResult<()> {
        self.module
            .signatures
            .push(Signature::from_wasm(wasm, clif));
        Ok(())
    }

    fn declare_func_import(
        &mut self,
        sig_index: SignatureIndex,
        module: &'data str,
        field: &'data str,
    ) -> WasmResult<()> {
        match self.env.import_function(module, field) {
            Some(func) => {
                // Check the returned Function signature matches the
                // requested import type
                func.signature
                    .check_wasm(&self.module.signatures[sig_index].wasm);

                let index = self.module.signatures.push(func.signature);
                self.module.functions.push(index);

                // Store the function name and pointer for the linker
                self.imported_functions
                    .push((format!("{}::{}", module, field), func.pointer));

                Ok(())
            }

            None => Err(WasmError::User(format!(
                "unknown function {} in module {}",
                field, module
            ))),
        }
    }

    fn declare_table_import(
        &mut self,
        table: Table,
        module: &'data str,
        field: &'data str,
    ) -> WasmResult<()> {
        panic!(
            "declare_table_import\n  {:?}\n  {:?}\n  {:?}",
            table, module, field
        )
    }

    fn declare_memory_import(
        &mut self,
        memory: Memory,
        module: &'data str,
        field: &'data str,
    ) -> WasmResult<()> {
        panic!(
            "declare_memory_import\n  {:?}\n  {:?}\n  {:?}",
            memory, module, field
        )
    }

    fn declare_global_import(
        &mut self,
        global: Global,
        module: &'data str,
        field: &'data str,
    ) -> WasmResult<()> {
        match self.env.import_global(module, field) {
            Some(value) => {
                match value {
                    GlobalValue::Const(_) => {
                        // Only externref constants are supported
                        if global.wasm_ty != WasmType::ExternRef {
                            return Err(WasmError::User(format!(
                                "invalid type for constant {}:{}, expected externref found {:?}",
                                module, field, global.wasm_ty
                            )));
                        }

                        if global.mutability {
                            return Err(WasmError::User(format!(
                                "invalid mutability for constant {}:{}",
                                module, field,
                            )));
                        }
                    }
                }

                self.module.globals.push(value);
                Ok(())
            }

            None => Err(WasmError::User(format!(
                "unknown global {} in module {}",
                field, module
            ))),
        }
    }

    fn declare_func_type(&mut self, sig_index: SignatureIndex) -> WasmResult<()> {
        self.module.functions.push(sig_index);
        Ok(())
    }

    fn declare_table(&mut self, _table: Table) -> WasmResult<()> {
        Ok(())
    }

    fn declare_memory(&mut self, memory: Memory) -> WasmResult<()> {
        self.memories.push(memory);
        Ok(())
    }

    fn declare_global(&mut self, _global: Global) -> WasmResult<()> {
        Ok(())
    }

    fn declare_func_export(&mut self, _func_index: FuncIndex, _name: &'data str) -> WasmResult<()> {
        Ok(())
    }

    fn declare_table_export(
        &mut self,
        _table_index: TableIndex,
        _name: &'data str,
    ) -> WasmResult<()> {
        Ok(())
    }

    fn declare_memory_export(
        &mut self,
        _memory_index: MemoryIndex,
        _name: &'data str,
    ) -> WasmResult<()> {
        Ok(())
    }

    fn declare_global_export(
        &mut self,
        _global_index: GlobalIndex,
        _name: &'data str,
    ) -> WasmResult<()> {
        Ok(())
    }

    fn declare_start_func(&mut self, index: FuncIndex) -> WasmResult<()> {
        self.start_func = Some(index);
        Ok(())
    }

    fn declare_table_elements(
        &mut self,
        _table_index: TableIndex,
        _base: Option<GlobalIndex>,
        _offset: usize,
        _elements: Box<[FuncIndex]>,
    ) -> WasmResult<()> {
        Ok(())
    }

    fn declare_passive_element(
        &mut self,
        index: ElemIndex,
        elements: Box<[FuncIndex]>,
    ) -> WasmResult<()> {
        trace!("declare_passive_element\n  {:?}\n  {:?}", index, elements);
        Ok(())
    }

    fn declare_passive_data(&mut self, data_index: DataIndex, data: &'data [u8]) -> WasmResult<()> {
        trace!("declare_passive_data\n  {:?}\n  {:?}", data_index, data);
        Ok(())
    }

    fn define_function_body(
        &mut self,
        _module_translation_state: &ModuleTranslationState,
        body_bytes: &'data [u8],
        body_offset: usize,
    ) -> WasmResult<()> {
        self.defined_functions.push(FunctionBody {
            body_bytes,
            body_offset,
        });
        Ok(())
    }

    fn declare_data_initialization(
        &mut self,
        memory_index: MemoryIndex,
        base: Option<GlobalIndex>,
        offset: usize,
        data: &'data [u8],
    ) -> WasmResult<()> {
        self.data_initializations[memory_index] = DataInitialization { base, offset, data };
        Ok(())
    }
}
