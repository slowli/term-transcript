//! Misc utils.

use std::io;

#[cfg(not(windows))]
pub(crate) fn is_recoverable_kill_error(err: &io::Error) -> bool {
    matches!(err.kind(), io::ErrorKind::InvalidInput)
}

// As per `TerminateProcess` docs (`TerminateProcess` is used by `Child::kill()`),
// the call will result in ERROR_ACCESS_DENIED if the process has already terminated.
//
// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess
#[cfg(windows)]
pub(crate) fn is_recoverable_kill_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied
    )
}
