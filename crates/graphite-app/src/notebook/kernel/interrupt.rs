//! Interrupting a kernel process that interrupts on a signal.
//!
//! On Unix that is SIGINT. On Windows kernels wait on an event whose handle they get in the
//! `JPY_INTERRUPT_EVENT` environment variable, so the event is created before the kernel starts.

/// What can interrupt a kernel process.
pub(crate) struct Interrupt {
    #[cfg(windows)]
    event: usize,
}

#[cfg(windows)]
impl Interrupt {
    /// Creates the event a Windows kernel waits on; its inheritable handle goes to the kernel
    /// through the environment.
    pub(crate) fn create() -> Option<Self> {
        use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
        use windows_sys::Win32::System::Threading::CreateEventW;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: 1,
        };
        // SAFETY: the attributes live for the call and the name is null (an unnamed event).
        let event = unsafe { CreateEventW(&attributes, 0, 0, std::ptr::null()) };
        (!event.is_null()).then_some(Self { event: event as usize })
    }

    /// The value for `JPY_INTERRUPT_EVENT`.
    pub(crate) fn environment(&self) -> Option<(String, String)> {
        Some(("JPY_INTERRUPT_EVENT".into(), self.event.to_string()))
    }

    pub(crate) fn signal(&self, _process: u32) {
        use windows_sys::Win32::System::Threading::SetEvent;
        // SAFETY: the handle is a valid event owned by `self`.
        unsafe {
            SetEvent(self.event as *mut core::ffi::c_void);
        }
    }
}

#[cfg(windows)]
impl Drop for Interrupt {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        // SAFETY: the handle is owned by `self` and closed once.
        unsafe {
            CloseHandle(self.event as *mut core::ffi::c_void);
        }
    }
}

#[cfg(not(windows))]
impl Interrupt {
    pub(crate) fn create() -> Option<Self> {
        Some(Self {})
    }

    pub(crate) fn environment(&self) -> Option<(String, String)> {
        None
    }

    pub(crate) fn signal(&self, process: u32) {
        let _ = std::process::Command::new("kill")
            .args(["-s", "INT", &process.to_string()])
            .status();
    }
}

/// Ends a process and, on Windows, the processes it started (a virtual environment's `python`
/// is a launcher that starts the real one).
pub(crate) fn kill_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}
