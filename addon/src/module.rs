use std::{
    ffi::CStr,
    sync::{Arc, Mutex},
};

use fabric_runtime::{with_abi, Environment, ExternRef, FuncRef, Function, GlobalValue, VMContext};
use log::{debug, log, warn, Level};

use crate::manager::{GameEvent, ListenerFunc};

pub(crate) type Module = Arc<Mutex<VMContext<FabricEnv>>>;

/// Implementation of the WASM host environment for a Source addon DLL
pub(crate) struct FabricEnv {
    pub(crate) listeners: Vec<Listener>,
}

impl Environment for FabricEnv {
    fn import_function(&mut self, module: &str, name: &str) -> Option<Function> {
        match module {
            "GameEventsManager" => match name {
                "add_listener" => Some(Function::new(
                    add_listener as with_abi!(fn(*mut VMContext<FabricEnv>, FuncRef, i32, i32)),
                )),
                _ => None,
            },
            "GameEvent" => match name {
                "get_int" => Some(Function::new(
                    get_int as with_abi!(fn(*mut VMContext<FabricEnv>, ExternRef, i32) -> i32),
                )),
                "get_bool" => Some(Function::new(
                    get_bool as with_abi!(fn(*mut VMContext<FabricEnv>, ExternRef, i32) -> i32),
                )),
                _ => None,
            },
            "LoggingSystem" => match name {
                "log" => Some(Function::new(
                    print_log as with_abi!(fn(*mut VMContext<FabricEnv>, ExternRef, i32)),
                )),
                _ => None,
            },
            _ => None,
        }
    }

    fn import_global(&mut self, module: &str, name: &str) -> Option<GlobalValue> {
        match module {
            "LoggingSystem" => match name {
                "Level::Error" => Some(GlobalValue::Const(0)),
                "Level::Warn" => Some(GlobalValue::Const(1)),
                "Level::Info" => Some(GlobalValue::Const(2)),
                "Level::Debug" => Some(GlobalValue::Const(3)),
                "Level::Trace" => Some(GlobalValue::Const(4)),
                _ => None,
            },
            _ => None,
        }
    }
}

pub(crate) struct Listener {
    pub(crate) listener: ListenerFunc,
    pub(crate) event: String,
    pub(crate) server_side: bool,
}

with_abi! {
    fn add_listener(
        ctx: *mut VMContext<FabricEnv>,
        listener: FuncRef,
        event: i32,
        server_side: i32,
    ) {
        debug!("add_listener({:?}, {:?}, {}, {})", ctx, listener, event, server_side);

        let ctx = unsafe { &mut *ctx };

        let listener = match ctx.function(listener) {
            Some(listener) => listener.get(),
            None => {
                warn!("could not resolve {:?}", listener);
                return;
            }
        };

        let event = match ctx.memory.load::<CStr>(event as usize) {
            Ok(event) => event,
            Err(()) => {
                warn!("could not load event string at {}", event);
                return;
            }
        };

        let event: String = event.to_string_lossy().into();

        let env = &mut ctx.environment;
        env.listeners.push(Listener {
            listener,
            event,
            server_side: server_side != 0,
        });
    }
}

with_abi! {
    fn get_int(ctx: *mut VMContext<FabricEnv>, event: ExternRef, name: i32) -> i32 {
        let ctx = unsafe { &mut *ctx };

        let evt_id = event;
        let event = ctx.externs.get_extern_mut::<Box<dyn GameEvent>>(event);

        let name = match ctx.memory.load::<CStr>(name as usize) {
            Ok(name) => name,
            Err(()) => {
                warn!("could not load string at {}", name);
                return 0;
            }
        };

        let res = event.get_int(name, 0);
        debug!("get_int({:?}, {:?}) -> {}", evt_id, name, res);
        res
    }
}

with_abi! {
    fn get_bool(ctx: *mut VMContext<FabricEnv>, event: ExternRef, name: i32) -> i32 {
        debug!("get_bool {:?} {:?}", event.0, name);

        let ctx = unsafe { &mut *ctx };

        let evt_id = event;
        let event = ctx.externs.get_extern_mut::<Box<dyn GameEvent>>(event);

        let name = match ctx.memory.load::<CStr>(name as usize) {
            Ok(name) => name,
            Err(()) => {
                warn!("could not load string at {}", name);
                return 0;
            }
        };

        let res = event.get_bool(name, false);
        debug!("get_bool({:?}, {:?}) -> {:?}", evt_id, name, res);
        if res { 1 } else { 0 }
    }
}

with_abi! {
    fn print_log(ctx: *mut VMContext<FabricEnv>, level: ExternRef, value: i32) {
        let ctx = unsafe { &mut *ctx };

        let level = match level.value() {
            0 => Level::Error,
            1 => Level::Warn,
            2 => Level::Info,
            3 => Level::Debug,
            4 => Level::Trace,
            level => {
                warn!("invalid logging level {}", level);
                return;
            }
        };

        let message = match ctx.memory.load::<CStr>(value as usize) {
            Ok(message) => message,
            Err(()) => {
                warn!("could not load message at {}", value);
                return;
            },
        };

        log!(level, "{}", message.to_string_lossy());
    }
}
