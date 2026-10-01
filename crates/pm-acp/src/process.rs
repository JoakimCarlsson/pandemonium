//! Operating system containment for an agent and the processes it starts.

use pm_host::{Child, Command};
use std::io;

#[cfg(unix)]
use std::sync::{Mutex, OnceLock};
#[cfg(unix)]
use std::thread::JoinHandle;

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};

/// Shutdown workers that must finish before the editor exits normally.
#[cfg(unix)]
static REAPERS: OnceLock<Mutex<Vec<JoinHandle<()>>>> = OnceLock::new();

/// Waits for every agent group to be killed and reaped at process exit.
#[cfg(unix)]
extern "C" fn wait_for_reapers() {
    if let Some(reapers) = REAPERS.get() {
        let mut reapers = reapers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for reaper in reapers.drain(..) {
            let _ = reaper.join();
        }
    }
}

/// Keeps a shutdown worker alive when the editor exits before its grace period.
pub(super) fn finish(worker: std::thread::JoinHandle<()>) {
    #[cfg(unix)]
    {
        let reapers = REAPERS.get_or_init(|| {
            unsafe { libc::atexit(wait_for_reapers) };
            Mutex::new(Vec::new())
        });
        reapers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(worker);
    }
    #[cfg(windows)]
    drop(worker);
}

/// Places a newly spawned agent in a process group of its own on Unix.
pub(super) fn configure(command: &mut Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    #[cfg(not(unix))]
    let _ = command;
}

/// The group or job that contains an agent's descendants.
pub(super) struct Containment {
    /// The group identifier, which is the direct child's pid.
    #[cfg(unix)]
    group: i32,
    /// A job that ends every assigned process when its handle closes.
    #[cfg(windows)]
    job: Option<OwnedHandle>,
}

impl Containment {
    /// Takes ownership of the operating system container for `process`.
    pub(super) fn new(process: &Child) -> io::Result<Self> {
        #[cfg(unix)]
        {
            Ok(Self {
                group: process.id() as i32,
            })
        }
        #[cfg(windows)]
        {
            let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = unsafe { OwnedHandle::from_raw_handle(raw) };
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    std::mem::size_of_val(&limits) as u32,
                )
            };
            if set == 0 {
                return Err(io::Error::last_os_error());
            }
            let assigned =
                unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) };
            if assigned == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { job: Some(job) })
        }
    }

    /// Returns a placeholder after moving the live container to a worker.
    pub(super) fn empty() -> Self {
        Self {
            #[cfg(unix)]
            group: 0,
            #[cfg(windows)]
            job: None,
        }
    }

    /// Asks every member of the group to end gracefully.
    pub(super) fn terminate(&self) {
        #[cfg(unix)]
        if self.group != 0 {
            unsafe { libc::kill(-self.group, libc::SIGTERM) };
        }
    }

    /// Ends every remaining member of the group or job.
    pub(super) fn kill(&self) {
        #[cfg(unix)]
        if self.group != 0 {
            unsafe { libc::kill(-self.group, libc::SIGKILL) };
        }
        #[cfg(windows)]
        if let Some(job) = &self.job {
            unsafe { TerminateJobObject(job.as_raw_handle(), 1) };
        }
    }
}
