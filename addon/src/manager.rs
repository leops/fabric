use std::{
    ffi::{c_void, CStr},
    os::raw::{c_char, c_int, c_uchar},
};

use fabric_runtime::{with_abi, ExternRef, VMContext};
use log::info;

use crate::module::{FabricEnv, Module};

#[fabric_codegen::interface]
pub(crate) trait GameEvent {
    fn destructor(&self);
    /// get event name
    fn get_name(&self) -> &CStr;

    /// if event handled reliable
    fn is_reliable(&self) -> bool;
    /// if event is never networked
    fn is_local(&self) -> bool;
    /// check if data field exists
    fn is_empty(&mut self, name: &CStr) -> bool;

    // Data access
    fn get_bool(&mut self, name: &CStr, default: bool) -> bool;
    fn get_int(&mut self, name: &CStr, default: c_int) -> c_int;
    fn get_uint64(&mut self, name: &CStr, default: u64) -> u64;
    fn get_float(&mut self, name: &CStr, default: f32) -> f32;
    fn get_string(&mut self, name: &CStr, default: &CStr) -> &CStr;

    fn set_bool(&mut self, name: &CStr, value: bool);
    fn set_int(&mut self, name: &CStr, value: c_int);
    fn set_uint64(&mut self, name: &CStr, value: u64);
    fn set_float(&mut self, name: &CStr, value: f32);
    fn set_string(&mut self, name: &CStr, value: &CStr);
}

#[repr(C)]
#[allow(dead_code)]
pub(crate) struct bf_write {
    /// The current buffer.
    data: *mut c_uchar,
    data_bytes: c_int,
    data_bits: c_int,

    /// Where we are in the buffer.
    cur_bit: c_int,

    /// Errors?
    overflow: bool,

    assert_on_overflow: bool,
    debug_name: *const c_char,
}

#[allow(non_camel_case_types)]
type bf_read = c_void;

#[fabric_codegen::interface]
pub(crate) trait GameEventManager2 {
    fn destructor(&self);

    // load game event descriptions from a file eg "resource\gameevents.res"
    fn load_events_from_file(&mut self, file_name: &CStr) -> c_int;

    // removes all and anything
    fn reset(&mut self);

    // adds a listener for a particular event
    fn add_listener(
        &mut self,
        listener: Box<dyn GameEventListener2>,
        name: &CStr,
        server_side: bool,
    ) -> bool;

    // returns true if this listener is listens to given event
    fn find_listener(&mut self, listener: &mut dyn GameEventListener2, name: &CStr) -> bool;

    // removes a listener
    fn remove_listener(&mut self, listener: &mut dyn GameEventListener2);

    // create an event by name, but doesn't fire it. returns NULL is event is not
    // known or no listener is registered for it. bForce forces the creation even if no listener is active
    fn create_event(&mut self, name: &CStr, force: bool, cookie: *mut c_int) -> Box<dyn GameEvent>;

    // fires a server event created earlier, if bDontBroadcast is set, event is not send to clients
    fn fire_event(&mut self, event: &mut dyn GameEvent, dont_broadcast: bool) -> bool;

    // fires an event for the local client only, should be used only by client code
    fn fire_event_client_side(&mut self, event: &mut dyn GameEvent) -> bool;

    // create a new copy of this event, must be free later
    fn duplicate_event(&mut self, event: &mut dyn GameEvent) -> Box<dyn GameEvent>;

    // if an event was created but not fired for some reason, it has to bee freed, same UnserializeEvent
    fn free_event(&mut self, event: &mut dyn GameEvent);

    // write/read event to/from bitbuffer
    fn serialize_event(&mut self, event: &mut dyn GameEvent, buf: *mut bf_write) -> bool;
    // create new KeyValues, must be deleted
    fn unserialize_event(&mut self, buf: *mut bf_read) -> Box<dyn GameEvent>;
}

#[fabric_codegen::interface]
pub(crate) trait GameEventListener2 {
    fn destructor(&self);

    /// FireEvent is called by EventManager if event just occured
    /// KeyValue memory will be freed by manager if not needed anymore
    fn fire_game_event(&mut self, event: Box<dyn GameEvent>);

    fn get_event_debug_id(&mut self) -> c_int;
}

pub(crate) type ListenerFunc = with_abi!(fn(*mut VMContext<FabricEnv>, ExternRef));

/// Wrapper implementing GameEventListener2 for a listener function declared in WASM,
pub(crate) struct FabricListener {
    pub(crate) module: Module,
    pub(crate) listener: ListenerFunc,
}

impl GameEventListener2 for FabricListener {
    fn destructor(&self) {
        info!("destructor");
    }

    fn fire_game_event(&mut self, event: Box<dyn GameEvent>) {
        info!("fire_game_event {:?}", event.get_name().to_string_lossy());

        let mut lock = self.module.lock().unwrap();
        let handle = lock.externs.create_extern(event);

        (self.listener)(&mut *lock, handle);

        lock.externs.take_extern::<Box<dyn GameEvent>>(handle);
    }

    fn get_event_debug_id(&mut self) -> c_int {
        42
    }
}
