(module
    (import "LoggingSystem" "Level::Info" (global externref))
    (import "LoggingSystem" "log" (func $log (param externref) (param i32)))

    (import "GameEventsManager" "add_listener" (func $add_listener (param funcref) (param i32) (param i32)))
    (import "GameEvent" "get_int" (func $get_int (param externref) (param i32) (result i32)))
    (import "GameEvent" "get_bool" (func $get_bool (param externref) (param i32) (result i32)))

    (table (export "__indirect_function_table") funcref
        (elem $on_portal_fired))
    
    (func $on_portal_fired (param $event externref)
        global.get 0
        i32.const 13
        call $log
        local.get $event
        i32.const 29
        call $get_int
        drop
        local.get $event
        i32.const 36
        call $get_bool
        drop)

    (func $start
        ref.func $on_portal_fired
        i32.const 0
        i32.const 1
        call $add_listener)

    (start $start)

    (memory (export "memory")
        (data "portal_fired\00on_portal_fired\00userid\00leftportal\00"))
)
