//! Local, opt-in, single-thread acceptance observation. Never carries library data.
use std::cell::RefCell;
use std::io;
use std::time::Instant;

use crate::{SourceVaultError, SourceVaultErrorCode};
mod file;
pub use file::TraceFile;
#[cfg(test)]
pub(crate) mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationError;

#[derive(Debug)]
pub enum TraceFailure<E> {
    Business(E),
    Observation(ObservationError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Operation {
    Action,
    Entry,
    Release,
    Read1,
    Read2,
    Label,
    #[cfg(target_os = "macos")]
    AttributesPre,
    #[cfg(target_os = "macos")]
    AttributesPost,
    #[cfg(target_os = "macos")]
    AttributesLabel,
    #[cfg(target_os = "macos")]
    AttributesSearch,
    #[cfg(any(target_os = "macos", test))]
    SecretSearch,
    #[cfg(any(target_os = "macos", test))]
    Domain,
}

trait EventSink {
    fn emit(&mut self, event: &[u8]) -> io::Result<()>;
}
struct Context {
    sink: Box<dyn EventSink>,
    started: Instant,
    stack: Vec<(u32, Operation)>,
    seq: u32,
    next_call: u32,
    bytes: usize,
    blocked: bool,
    broken: bool,
}
thread_local! {
    static ACTIVE: RefCell<Option<Context>> = const { RefCell::new(None) };
}
struct CallGuard {
    finished: bool,
}
impl Drop for CallGuard {
    fn drop(&mut self) {
        if !self.finished {
            ACTIVE.with(|active| {
                if let Some(context) = active.borrow_mut().as_mut() {
                    context.broken = true;
                    context.blocked = true;
                }
            });
        }
    }
}
fn reject_nested() -> bool {
    ACTIVE.with(|active| {
        if let Some(context) = active.borrow_mut().as_mut() {
            context.blocked = true;
            context.broken = true;
            true
        } else {
            false
        }
    })
}
struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            active.borrow_mut().take();
        });
    }
}
fn failed() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::KeyReadObservationFailed,
        "key read observation failed",
    )
}
fn category(error: &SourceVaultError) -> &'static str {
    match error.code() {
        SourceVaultErrorCode::KeyMissing => "missing",
        SourceVaultErrorCode::KeyAmbiguous => "ambiguous",
        SourceVaultErrorCode::KeyStoreDenied | SourceVaultErrorCode::KeyStoreLocked => "denied",
        SourceVaultErrorCode::KeyStoreCancelled => "cancelled",
        SourceVaultErrorCode::KeyStoreUnavailable => "unavailable",
        SourceVaultErrorCode::KeyCorrupt
        | SourceVaultErrorCode::KeyMetadataMismatch
        | SourceVaultErrorCode::KeyReadbackMismatch => "invalid",
        _ => "failed",
    }
}
impl Context {
    fn emit(
        &mut self,
        id: u32,
        operation: Operation,
        edge: &'static str,
        result: &'static str,
    ) -> Result<(), ObservationError> {
        if self.broken {
            return Err(ObservationError);
        }
        let parent = self
            .stack
            .iter()
            .rev()
            .find(|(call, _)| *call != id)
            .map_or(0, |(call, _)| *call);
        let stage = self
            .stack
            .iter()
            .rev()
            .find_map(|(_, op)| match op {
                Operation::Entry => Some("entry"),
                Operation::Release => Some("release"),
                _ => None,
            })
            .unwrap_or("none");
        let read = self
            .stack
            .iter()
            .rev()
            .find_map(|(_, op)| match op {
                Operation::Read1 => Some(1),
                Operation::Read2 => Some(2),
                _ => None,
            })
            .unwrap_or(0);
        let line = format!(
            "{{\"version\":1,\"action\":1,\"seq\":{},\"call\":{id},\"parent\":{parent},\"stage\":\"{stage}\",\"read\":{read},\"operation\":\"{operation:?}\",\"edge\":\"{edge}\",\"elapsed_ns\":{},\"result\":\"{result}\"}}\n",
            self.seq + 1,
            self.started.elapsed().as_nanos()
        );
        if self.seq >= 256
            || self.bytes + line.len() > 64 * 1024
            || self.sink.emit(line.as_bytes()).is_err()
        {
            self.blocked = true;
            self.broken = true;
            return Err(ObservationError);
        }
        self.seq += 1;
        self.bytes += line.len();
        Ok(())
    }
    fn begin(&mut self, operation: Operation) -> Result<u32, ObservationError> {
        if self.blocked || self.broken {
            return Err(ObservationError);
        }
        self.next_call += 1;
        let id = self.next_call;
        self.stack.push((id, operation));
        self.emit(id, operation, "begin", "pending")?;
        Ok(id)
    }
    fn end(
        &mut self,
        id: u32,
        operation: Operation,
        result: &'static str,
    ) -> Result<(), ObservationError> {
        if self.stack.last() != Some(&(id, operation)) {
            self.broken = true;
            self.blocked = true;
            return Err(ObservationError);
        }
        let outcome = self.emit(id, operation, "end", result);
        self.stack.pop();
        if result != "ok" {
            self.blocked = true;
        }
        outcome
    }
}

/// Creates the exclusive trace only when the caller is ready to execute its one action.
/// Business errors retain their original value even if the trace also fails.
pub fn with_trace<T, E>(
    trace: TraceFile,
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, TraceFailure<E>> {
    if reject_nested() {
        return Err(TraceFailure::Observation(ObservationError));
    }
    let sink = trace.create().map_err(TraceFailure::Observation)?;
    with_sink(Box::new(sink), operation)
}
fn with_sink<T, E>(
    sink: Box<dyn EventSink>,
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, TraceFailure<E>> {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        if let Some(context) = active.as_mut() {
            context.blocked = true;
            context.broken = true;
            return Err(TraceFailure::Observation(ObservationError));
        }
        *active = Some(Context {
            sink,
            started: Instant::now(),
            stack: Vec::new(),
            seq: 0,
            next_call: 0,
            bytes: 0,
            blocked: false,
            broken: false,
        });
        Ok(())
    })?;
    let _scope = Scope;
    let id = ACTIVE
        .with(|active| {
            active
                .borrow_mut()
                .as_mut()
                .unwrap()
                .begin(Operation::Action)
        })
        .map_err(TraceFailure::Observation)?;
    let result = operation();
    let end = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let context = active.as_mut().unwrap();
        let classification = if result.is_ok() && !context.blocked {
            "ok"
        } else {
            "business_failed"
        };
        let outcome = context.end(id, Operation::Action, classification);
        if context.broken || (result.is_ok() && context.blocked) {
            Err(ObservationError)
        } else {
            outcome
        }
    });
    match result {
        Err(error) => Err(TraceFailure::Business(error)),
        Ok(value) => end.map(|()| value).map_err(TraceFailure::Observation),
    }
}

pub(crate) fn call<T>(
    operation: Operation,
    work: impl FnOnce() -> Result<T, SourceVaultError>,
) -> Result<T, SourceVaultError> {
    let id = ACTIVE.with(|active| {
        active
            .borrow_mut()
            .as_mut()
            .map(|context| context.begin(operation))
    });
    let Some(id) = id else {
        return work();
    };
    let id = id.map_err(|_| failed())?;
    let mut guard = CallGuard { finished: false };
    // Never hold a TLS borrow over a system call or a nested observed helper.
    let result = work();
    let end = ACTIVE.with(|active| {
        active.borrow_mut().as_mut().unwrap().end(
            id,
            operation,
            result
                .as_ref()
                .map_or_else(|error| category(error), |_| "ok"),
        )
    });
    guard.finished = true;
    match result {
        Err(error) => Err(error),
        Ok(value) => end.map(|()| value).map_err(|_| failed()),
    }
}
