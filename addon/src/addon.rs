use std::{
    ffi::{c_void, CStr, CString},
    mem::swap,
    ops::{Deref, DerefMut},
    os::raw::{c_char, c_int, c_short},
    sync::{Arc, Mutex},
};

use fabric_codegen::cstr;
use fabric_runtime::load_module;
use log::{info, warn};

use crate::{
    foreign::{create_interface, CreateInterfaceFn},
    manager::{FabricListener, GameEventManager2},
    module::{FabricEnv, Module},
};

#[repr(C)]
#[derive(Debug)]
pub(crate) struct Edict {
    state_flags: c_int,
    edict_index: c_short,
    network_serial_number: c_short,
    networkable: *mut c_void,
    unk: *mut c_void,
    freetime: f32,
}

const COMMAND_MAX_ARGC: usize = 64;
const COMMAND_MAX_LENGTH: usize = 512;

#[repr(C)]
#[derive(Debug)]
pub(crate) struct CCommand {
    argc: c_int,
    argv0_size: c_int,
    arg_s_buffer: [c_char; COMMAND_MAX_LENGTH],
    argv_buffer: [c_char; COMMAND_MAX_LENGTH],
    argv: [*const c_char; COMMAND_MAX_ARGC],
}

#[repr(C)]
#[allow(dead_code)]
pub(crate) enum PluginResult {
    /// keep going
    Continue = 0,
    /// run the game dll function but use our return value instead
    Override,
    /// don't run the game dll function at all
    Stop,
}

type QueryCvarCookie = c_int;

#[repr(C)]
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum QueryCvarValueStatus {
    /// It got the value fine.
    ValueIntact = 0,
    CvarNotFound = 1,
    /// There's a ConCommand, but it's not a ConVar.
    NotACvar = 2,
    /// The cvar was marked with FCVAR_SERVER_CAN_NOT_QUERY, so the server is not allowed to have its value.
    CvarProtected = 3,
}

#[fabric_codegen::interface]
pub(crate) trait ServerPluginCallbacks {
    /// Initialize the plugin to run
    /// Return false if there is an error during startup.
    fn load(
        &mut self,
        interface_factory: CreateInterfaceFn,
        game_server_factory: CreateInterfaceFn,
    ) -> bool;

    /// Called when the plugin should be shutdown
    fn unload(&mut self);

    /// called when a plugins execution is stopped but the plugin is not unloaded
    fn pause(&mut self);

    /// called when a plugin should start executing again (sometime after a Pause() call)
    fn unpause(&mut self);

    /// Returns string describing current plugin.  e.g., Admin-Mod.
    fn get_plugin_description(&mut self) -> &CStr;

    /// Called any time a new level is started (after GameInit() also on level transitions within a game)
    fn level_init(&mut self, map_name: &CStr);

    /// The server is about to activate
    fn server_activate(&mut self, edict_list: *mut Edict, edict_count: c_int, client_max: c_int);

    /// The server should run physics/think on all edicts
    fn game_frame(&mut self, simulating: bool);

    /// Called when a level is shutdown (including changing levels)
    fn level_shutdown(&mut self);

    /// Client is going active
    fn client_active(&mut self, entity: *mut Edict);

    /// Client is fully connected ( has received initial baseline of entities )
    fn client_fully_connect(&mut self, entity: *mut Edict);

    /// Client is disconnecting from server
    fn client_disconnect(&mut self, entity: *mut Edict);

    /// Client is connected and should be put in the game
    fn client_put_in_server(&mut self, entity: *mut Edict, player_name: &CStr);

    /// Sets the client index for the client who typed the command into their console
    fn set_command_client(&mut self, index: c_int);

    /// A player changed one/several replicated cvars (name etc)
    fn client_settings_changed(&mut self, entity: *mut Edict);

    /// Client is connecting to server ( set retVal to false to reject the connection )
    /// You can specify a rejection message by writing it into reject
    fn client_connect(
        &mut self,
        allow_connect: *mut bool,
        entity: *mut Edict,
        name: &CStr,
        address: &CStr,
        reject: *mut c_char,
        max_reject_len: c_int,
    ) -> PluginResult;

    /// The client has typed a command at the console
    fn client_command(&mut self, entity: *mut Edict, args: *const CCommand) -> PluginResult;

    /// A user has had their network id setup and validated
    fn network_id_validated(&mut self, user_name: &CStr, network_id: &CStr) -> PluginResult;

    /// This is called when a query from IServerPluginHelpers::StartQueryCvarValue is finished.
    /// iCookie is the value returned by IServerPluginHelpers::StartQueryCvarValue.
    fn on_query_cvar_value_finished(
        &mut self,
        cookie: QueryCvarCookie,
        entity: *mut Edict,
        status: QueryCvarValueStatus,
        cvar_name: *mut c_char,
        cvar_value: *mut c_char,
    );

    fn on_edict_allocated(&mut self, edict: *mut Edict);
    fn on_edict_freed(&mut self, edict: *const Edict);
}

/// Main entry point object for the addon DLL
///
/// Loads a (static) WASM module on load and execute it
/// in the addon host environment
pub(crate) struct FabricAddon {
    modules: Vec<Module>,
}

impl ServerPluginCallbacks for FabricAddon {
    fn load(&mut self, factory: CreateInterfaceFn, server: CreateInterfaceFn) -> bool {
        info!("load {:?} {:?}", factory, server);

        if let Some(mut manager) =
            create_interface::<dyn GameEventManager2>(factory, cstr!("GAMEEVENTSMANAGER002"))
        {
            static SOURCE: &str = include_str!("../example.wat");

            let mut module = load_module(
                FabricEnv {
                    listeners: Vec::new(),
                },
                SOURCE,
            );

            // The `listeners` list wont be needed anymore in the environment,
            // swap it with an empty one and consume it in the initialization loop
            let mut listeners = Vec::new();
            swap(&mut module.environment.listeners, &mut listeners);

            let module = Arc::new(Mutex::new(module));

            for listener in listeners {
                let event = match CString::new(listener.event.as_bytes()) {
                    Ok(event) => event,
                    Err(err) => {
                        warn!("CString::new({:?}): {}", listener.event, err);
                        continue;
                    }
                };

                let is_ok = manager.add_listener(
                    Box::new(FabricListener {
                        module: module.clone(),
                        listener: listener.listener,
                    }),
                    &event,
                    listener.server_side,
                );

                if !is_ok {
                    warn!("could not add event listener for {}", listener.event);
                }
            }

            self.modules.push(module);
        } else {
            warn!("GAMEEVENTSMANAGER002 not found");
        }

        true
    }

    fn unload(&mut self) {
        self.modules.clear();
    }

    fn pause(&mut self) {}

    fn unpause(&mut self) {}

    fn get_plugin_description(&mut self) -> &CStr {
        cstr!("Fabric")
    }

    fn level_init(&mut self, _map_name: &CStr) {}

    fn server_activate(
        &mut self,
        _edict_list: *mut Edict,
        _edict_count: c_int,
        _client_max: c_int,
    ) {
    }

    fn game_frame(&mut self, _simulating: bool) {}

    fn level_shutdown(&mut self) {}

    fn on_query_cvar_value_finished(
        &mut self,
        _cookie: QueryCvarCookie,
        _entity: *mut Edict,
        _status: QueryCvarValueStatus,
        _var_name: *mut c_char,
        _var_value: *mut c_char,
    ) {
    }

    fn on_edict_allocated(&mut self, _entity: *mut Edict) {}

    fn on_edict_freed(&mut self, _entity: *const Edict) {}

    fn client_active(&mut self, _entity: *mut Edict) {}

    fn client_fully_connect(&mut self, _entity: *mut Edict) {}

    fn client_disconnect(&mut self, _entity: *mut Edict) {}

    fn client_put_in_server(&mut self, _entity: *mut Edict, _player_name: &CStr) {}

    fn set_command_client(&mut self, _index: c_int) {}

    fn client_settings_changed(&mut self, _entity: *mut Edict) {}

    fn client_connect(
        &mut self,
        _allow_connect: *mut bool,
        _entity: *mut Edict,
        _name: &CStr,
        _address: &CStr,
        _reject: *mut c_char,
        _max_reject_len: c_int,
    ) -> PluginResult {
        PluginResult::Continue
    }

    fn client_command(&mut self, _entity: *mut Edict, _args: *const CCommand) -> PluginResult {
        PluginResult::Continue
    }

    fn network_id_validated(&mut self, _user_name: &CStr, _network_id: &CStr) -> PluginResult {
        PluginResult::Continue
    }
}

impl Deref for FabricAddon {
    type Target = Self;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl DerefMut for FabricAddon {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self
    }
}

static VTABLE: IServerPluginCallbacks =
    <dyn ServerPluginCallbacks>::vtable::<FabricAddon, FabricAddon>();

pub(crate) static mut INSTANCE: CServerPluginCallbacks<FabricAddon> = CServerPluginCallbacks {
    vtable: &VTABLE,
    instance: FabricAddon {
        modules: Vec::new(),
    },
};
