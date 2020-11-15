#![allow(non_camel_case_types, dead_code)]

use std::{
    ffi::CString,
    os::raw::{c_char, c_int, c_uint},
    panic::{set_hook, PanicInfo},
};

use fabric_codegen::cstr;
use log::{set_logger_racy, set_max_level, trace, Level, LevelFilter, Log, Metadata, Record};

type LoggingChannelID = c_int;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
enum LoggingSeverity {
    ///-----------------------------------------------------------------------------
    /// An informative logging message.
    ///-----------------------------------------------------------------------------
    Message = 0,

    ///-----------------------------------------------------------------------------
    /// A warning, typically non-fatal
    ///-----------------------------------------------------------------------------
    Warning = 1,

    ///-----------------------------------------------------------------------------
    /// A message caused by an Assert**() operation.
    ///-----------------------------------------------------------------------------
    Assert = 2,

    ///-----------------------------------------------------------------------------
    /// An error, typically fatal/unrecoverable.
    ///-----------------------------------------------------------------------------
    Error = 3,

    ///-----------------------------------------------------------------------------
    /// A placeholder level, higher than any legal value.
    /// Not a real severity value!
    ///-----------------------------------------------------------------------------
    HighestSeverity = 4,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
enum LoggingResponse {
    Continue,
    Debugger,
    Abort,
}

type RegisterTagsFunc = extern "C" fn();

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct Color {
    _color: [c_uint; 4],
}

// For linking purpose the symbols are loaded from the Alien Swarm SDK,
// at runtime this will call into the tier0.dll of whatever game loaded this
#[link(
    name = "D:/SteamLibrary/SteamApps/common/Alien Swarm/sdk_src/lib/public/tier0",
    kind = "dylib"
)]
extern "C" {
    fn LoggingSystem_RegisterLoggingChannel(
        pName: *const c_char,
        registerTagsFunc: RegisterTagsFunc,
        flags: c_int,
        severity: LoggingSeverity,
        color: Color,
    ) -> LoggingChannelID;

    fn LoggingSystem_Log(
        channelID: LoggingChannelID,
        severity: LoggingSeverity,
        pMessageFormat: *const c_char,
    ) -> LoggingResponse;
}

struct Logger(LoggingChannelID);

impl Logger {
    /// Print `message` into the logger's channel at `severity` level
    ///
    /// If message is too long it will be split into several successive call
    /// to the logging function
    fn print(&self, severity: LoggingSeverity, mut message: &str) {
        while !message.is_empty() {
            let mut index = message.len().min(254);
            while !message.is_char_boundary(index) {
                index -= 1;
            }

            let (head, tail) = message.split_at(index);
            message = tail;

            if let Ok(line) = CString::new(head) {
                unsafe {
                    LoggingSystem_Log(self.0, severity, line.as_ptr());
                }
            }
        }
    }
}

impl Log for Logger {
    fn enabled(&self, _meta: &Metadata) -> bool {
        true
        // let target = meta.target();
        // target.starts_with("fabric")
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let line = format!(
            "[{} {}] {}\n",
            record.level(),
            record.target(),
            record.args()
        );

        let severity = match record.level() {
            Level::Error => LoggingSeverity::Warning,
            Level::Warn => LoggingSeverity::Warning,
            Level::Info => LoggingSeverity::Message,
            Level::Debug => LoggingSeverity::Message,
            Level::Trace => LoggingSeverity::Message,
        };

        self.print(severity, &line);
    }

    fn flush(&self) {}
}

static mut LOGGER: Logger = Logger(0);

fn log_panic(info: &PanicInfo) {
    let logger = unsafe { &LOGGER };
    logger.print(LoggingSeverity::Error, &info.to_string());
}

/// Initialize the logging facade
///
/// Acquires a logging channel from the engine and register
/// it to the log function. Finally, registers a panic hook
/// that logs the panic infos at error level.
pub(crate) fn init_logger() {
    extern "C" fn register() {}

    unsafe {
        LOGGER.0 = LoggingSystem_RegisterLoggingChannel(
            cstr!("fabric").as_ptr(),
            register,
            0,
            LoggingSeverity::Message,
            Color { _color: [0; 4] },
        );
    }

    if let Err(err) = unsafe { set_logger_racy(&LOGGER) } {
        println!("Failed to set logger: {:?}", err);
    } else {
        set_max_level(LevelFilter::Debug);
        trace!("Logger initialized");
    }

    set_hook(Box::new(log_panic));
}
