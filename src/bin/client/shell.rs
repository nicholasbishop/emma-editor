use {
    crate::shell_unix::Pty,
    anyhow::Error,
    fehler::throws,
    gtk4::glib::{self, ffi as glib_sys},
    nix::libc,
    std::{fmt, io::Write},
};

#[allow(non_camel_case_types)]
type gsize = libc::size_t;

// TODO: generate wrappers for gio channel stuff in the upstream lib

unsafe extern "C" fn on_output_ready_wrapper(
    channel: *mut glib_sys::GIOChannel,
    _condition: glib_sys::GIOCondition,
    data: glib_sys::gpointer,
) -> i32 {
    // TODO: avoid zeroing?
    let mut buf = [0; 4096];
    let mut bytes_read: gsize = 0;
    let mut err: *mut glib_sys::GError = std::ptr::null_mut();

    let status = glib_sys::g_io_channel_read_chars(
        channel,
        buf.as_mut_ptr(),
        buf.len(),
        &mut bytes_read as *mut gsize,
        &mut err,
    );

    if status != glib_sys::G_IO_STATUS_NORMAL {
        // TODO
        dbg!(status);
    }

    let shell: *mut ShellInternal = data.cast();
    ((*shell).on_output_ready)(&buf[0..bytes_read]);

    // Return value of non-zero indicates event source should be kept.
    1
}

pub type OnOutputReady = Box<dyn FnMut(&[u8])>;

struct ShellInternal {
    on_output_ready: OnOutputReady,
    pty: Pty,
}

// Box the actual data so that the C callback can safely access the
// pointer.
pub struct Shell(Box<ShellInternal>);

impl Shell {
    #[throws]
    pub fn launch(on_output_ready: OnOutputReady) -> Shell {
        let mut shell = Box::new(ShellInternal {
            on_output_ready,
            pty: Pty::new(),
        });

        let shell_ptr: *mut ShellInternal = &mut *shell;

        unsafe {
            let channel = glib_sys::g_io_channel_unix_new(shell.pty.raw_fd());
            // Returns the event source id, TODO add to struct
            // and remove on drop?
            let event_source_id = glib_sys::g_io_add_watch(
                channel,
                glib::IOCondition::IN.bits(),
                Some(on_output_ready_wrapper),
                shell_ptr.cast(),
            );
            dbg!(event_source_id);
        }

        Shell(shell)
    }

    #[throws]
    pub fn send(&mut self, data: &[u8]) {
        self.0.pty.file().write_all(data)?;
    }
}

impl fmt::Debug for Shell {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // TODO
        write!(f, "Shell {{ ... }}")
    }
}
