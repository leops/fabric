use cranelift_codegen::{
    cursor,
    ir::{self, ExtFuncData, ExternalName, Function, InstBuilder},
    isa::TargetFrontendConfig,
};
use cranelift_wasm::{
    FuncEnvironment, FuncIndex, FunctionBuilder, GlobalIndex, GlobalVariable, MemoryIndex,
    SignatureIndex, TableIndex, TargetEnvironment, WasmError, WasmResult,
};

use super::{
    module::ModuleDefs,
    signature::{ExternRef, CALL_CONV, POINTER_WIDTH},
    GlobalValue,
};

pub(crate) struct FunctionEnv<'module> {
    pub(crate) module: &'module ModuleDefs,
}

impl<'module> TargetEnvironment for FunctionEnv<'module> {
    fn target_config(&self) -> TargetFrontendConfig {
        TargetFrontendConfig {
            default_call_conv: CALL_CONV,
            pointer_width: POINTER_WIDTH,
        }
    }
}

impl<'module> FuncEnvironment for FunctionEnv<'module> {
    fn make_global(
        &mut self,
        _func: &mut Function,
        index: GlobalIndex,
    ) -> WasmResult<GlobalVariable> {
        match self.module.globals[index] {
            // Constants are declared as `Custom` so their value can be
            // defined inline in the emitted IR in `translate_custom_global_get`
            GlobalValue::Const(_) => Ok(GlobalVariable::Custom),
        }
    }

    fn make_heap(&mut self, _func: &mut Function, _index: MemoryIndex) -> WasmResult<ir::Heap> {
        panic!("make_heap")
    }

    fn make_table(&mut self, _func: &mut Function, _index: TableIndex) -> WasmResult<ir::Table> {
        panic!("make_table")
    }

    fn make_indirect_sig(
        &mut self,
        _func: &mut Function,
        _index: SignatureIndex,
    ) -> WasmResult<ir::SigRef> {
        panic!("make_indirect_sig")
    }

    fn make_direct_func(
        &mut self,
        func: &mut Function,
        index: FuncIndex,
    ) -> WasmResult<ir::FuncRef> {
        let sig = &self.module.signatures[self.module.functions[index]].clif;
        let signature = func.import_signature(sig.clone());
        Ok(func.import_function(ExtFuncData {
            name: ExternalName::user(0, index.as_u32()),
            signature,
            colocated: false,
        }))
    }

    fn translate_call_indirect(
        &mut self,
        _pos: cursor::FuncCursor,
        _table_index: TableIndex,
        _table: ir::Table,
        _sig_index: SignatureIndex,
        _sig_ref: ir::SigRef,
        _callee: ir::Value,
        _call_args: &[ir::Value],
    ) -> WasmResult<ir::Inst> {
        panic!("translate_call_indirect")
    }

    fn translate_memory_grow(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
        _val: ir::Value,
    ) -> WasmResult<ir::Value> {
        panic!("translate_memory_grow")
    }

    fn translate_memory_size(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
    ) -> WasmResult<ir::Value> {
        panic!("translate_memory_size")
    }

    fn translate_memory_copy(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
        _dst: ir::Value,
        _src: ir::Value,
        _len: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_memory_copy")
    }

    fn translate_memory_fill(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
        _dst: ir::Value,
        _val: ir::Value,
        _len: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_memory_fill")
    }

    fn translate_memory_init(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
        _seg_index: u32,
        _dst: ir::Value,
        _src: ir::Value,
        _len: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_memory_init")
    }

    fn translate_data_drop(&mut self, _pos: cursor::FuncCursor, _seg_index: u32) -> WasmResult<()> {
        panic!("translate_data_drop")
    }

    fn translate_table_size(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: TableIndex,
        _table: ir::Table,
    ) -> WasmResult<ir::Value> {
        panic!("translate_table_size")
    }

    fn translate_table_grow(
        &mut self,
        _pos: cursor::FuncCursor,
        _table_index: TableIndex,
        _table: ir::Table,
        _delta: ir::Value,
        _init_value: ir::Value,
    ) -> WasmResult<ir::Value> {
        panic!("translate_table_grow")
    }

    fn translate_table_get(
        &mut self,
        _builder: &mut FunctionBuilder,
        _table_index: TableIndex,
        _table: ir::Table,
        _index: ir::Value,
    ) -> WasmResult<ir::Value> {
        panic!("translate_table_get")
    }

    fn translate_table_set(
        &mut self,
        _builder: &mut FunctionBuilder,
        _table_index: TableIndex,
        _table: ir::Table,
        _value: ir::Value,
        _index: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_table_set")
    }

    fn translate_table_copy(
        &mut self,
        _pos: cursor::FuncCursor,
        _dst_table_index: TableIndex,
        _dst_table: ir::Table,
        _src_table_index: TableIndex,
        _src_table: ir::Table,
        _dst: ir::Value,
        _src: ir::Value,
        _len: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_table_copy")
    }

    fn translate_table_fill(
        &mut self,
        _pos: cursor::FuncCursor,
        _table_index: TableIndex,
        _dst: ir::Value,
        _val: ir::Value,
        _len: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_table_fill")
    }

    fn translate_table_init(
        &mut self,
        _pos: cursor::FuncCursor,
        _seg_index: u32,
        _table_index: TableIndex,
        _table: ir::Table,
        _dst: ir::Value,
        _src: ir::Value,
        _len: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_table_init")
    }

    fn translate_elem_drop(&mut self, _pos: cursor::FuncCursor, _seg_index: u32) -> WasmResult<()> {
        panic!("translate_elem_drop")
    }

    fn translate_ref_func(
        &mut self,
        mut pos: cursor::FuncCursor,
        func_index: FuncIndex,
    ) -> WasmResult<ir::Value> {
        let index = func_index.as_u32() as i64;
        Ok(pos.ins().iconst(ir::types::I32, index))
    }

    fn translate_custom_global_get(
        &mut self,
        mut pos: cursor::FuncCursor,
        index: GlobalIndex,
    ) -> WasmResult<ir::Value> {
        match self.module.globals[index] {
            GlobalValue::Const(value) => {
                let value = ExternRef::from_const(value);
                Ok(pos.ins().iconst(ir::types::I64, value.0 as i64))
            }
        }
    }

    fn translate_custom_global_set(
        &mut self,
        _pos: cursor::FuncCursor,
        _global_index: GlobalIndex,
        _val: ir::Value,
    ) -> WasmResult<()> {
        panic!("translate_custom_global_set")
    }

    fn translate_atomic_wait(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
        _addr: ir::Value,
        _expected: ir::Value,
        _timeout: ir::Value,
    ) -> WasmResult<ir::Value> {
        panic!("translate_atomic_wait")
    }

    fn translate_atomic_notify(
        &mut self,
        _pos: cursor::FuncCursor,
        _index: MemoryIndex,
        _heap: ir::Heap,
        _addr: ir::Value,
        _count: ir::Value,
    ) -> WasmResult<ir::Value> {
        panic!("translate_atomic_notify")
    }

    fn translate_call(
        &mut self,
        mut pos: cursor::FuncCursor,
        _callee_index: FuncIndex,
        callee: ir::FuncRef,
        call_args: &[ir::Value],
    ) -> WasmResult<ir::Inst> {
        // Prepend the vmtcx pointer to all function calls
        let ctx = match pos.func.special_param(ir::ArgumentPurpose::VMContext) {
            Some(ctx) => ctx,
            None => return Err(WasmError::User(String::from("missing vmtcx parameter"))),
        };

        let mut args = Vec::with_capacity(call_args.len() + 1);
        args.push(ctx);
        args.extend_from_slice(call_args);

        Ok(pos.ins().call(callee, &args))
    }
}
