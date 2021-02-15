// Modified copy of https://github.com/alacritty/alacritty/blob/a754d06b44139b85e8b34a71ece4477cb1caa85e/alacritty_terminal/src/tty/unix.rs
//

//! TTY related functionality.

use log::error;
use nix::libc::{self, c_int, winsize, TIOCSCTTY};
use nix::pty::openpty;
use nix::sys::termios::{self, InputFlags, SetArg};
use std::ffi::CStr;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::{
    io::{AsRawFd, FromRawFd, RawFd},
    process::CommandExt,
};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::ptr;

macro_rules! die {
    ($($arg:tt)*) => {{
        error!($($arg)*);
        std::process::exit(1);
    }}
}

pub struct Size {
    rows: usize,
    columns: usize,
    width_in_pixels: usize,
    height_in_pixels: usize,
}

/// Get raw fds for master/slave ends of a new PTY.
fn make_pty(size: &Size) -> (RawFd, RawFd) {
    let win_size = winsize {
        ws_row: size.rows as _,
        ws_col: size.columns as _,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let ends = openpty(Some(&win_size), None).expect("openpty failed");

    (ends.master, ends.slave)
}

/// Really only needed on BSD, but should be fine elsewhere.
fn set_controlling_terminal(fd: c_int) {
    let res = unsafe {
        // TIOSCTTY changes based on platform and the `ioctl` call is different
        // based on architecture (32/64). So a generic cast is used to make sure
        // there are no issues. To allow such a generic cast the clippy warning
        // is disabled.
        #[allow(clippy::cast_lossless)]
        libc::ioctl(fd, TIOCSCTTY as _, 0)
    };

    if res < 0 {
        die!("ioctl TIOCSCTTY failed: {}", io::Error::last_os_error());
    }
}

#[derive(Debug)]
struct Passwd<'a> {
    name: &'a str,
    passwd: &'a str,
    uid: libc::uid_t,
    gid: libc::gid_t,
    gecos: &'a str,
    dir: &'a str,
    shell: &'a str,
}

/// Return a Passwd struct with pointers into the provided buf.
///
/// # Unsafety
///
/// If `buf` is changed while `Passwd` is alive, bad thing will almost certainly happen.
fn get_pw_entry(buf: &mut [i8; 1024]) -> Passwd<'_> {
    // Create zeroed passwd struct.
    let mut entry: MaybeUninit<libc::passwd> = MaybeUninit::uninit();

    let mut res: *mut libc::passwd = ptr::null_mut();

    // Try and read the pw file.
    let uid = unsafe { libc::getuid() };
    let status = unsafe {
        libc::getpwuid_r(
            uid,
            entry.as_mut_ptr(),
            buf.as_mut_ptr() as *mut _,
            buf.len(),
            &mut res,
        )
    };
    let entry = unsafe { entry.assume_init() };

    if status < 0 {
        die!("getpwuid_r failed");
    }

    if res.is_null() {
        die!("pw not found");
    }

    // Sanity check.
    assert_eq!(entry.pw_uid, uid);

    // Build a borrowed Passwd struct.
    Passwd {
        name: unsafe { CStr::from_ptr(entry.pw_name).to_str().unwrap() },
        passwd: unsafe { CStr::from_ptr(entry.pw_passwd).to_str().unwrap() },
        uid: entry.pw_uid,
        gid: entry.pw_gid,
        gecos: unsafe { CStr::from_ptr(entry.pw_gecos).to_str().unwrap() },
        dir: unsafe { CStr::from_ptr(entry.pw_dir).to_str().unwrap() },
        shell: unsafe { CStr::from_ptr(entry.pw_shell).to_str().unwrap() },
    }
}

pub struct Pty {
    // TODO
    #[allow(dead_code)]
    child: Child,

    fd: File,
}

struct Program {
    program: PathBuf,
    argv: Vec<OsString>,
}

impl Pty {
    /// Create a new TTY and return a handle to interact with it.
    pub fn new() -> Pty {
        // TODO
        let size = Size {
            rows: 20,
            columns: 80,
            width_in_pixels: 400,
            height_in_pixels: 600,
        };

        let (master, slave) = make_pty(&size);

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if let Ok(mut termios) = termios::tcgetattr(master) {
            // Set character encoding to UTF-8.
            termios.input_flags.set(InputFlags::IUTF8, true);
            let _ = termios::tcsetattr(master, SetArg::TCSANOW, &termios);
        }

        let mut buf = [0; 1024];
        let pw = get_pw_entry(&mut buf);

        // TODO
        let shell = Program {
            program: "/bin/bash".into(),
            argv: vec!["-i".into()],
        };

        let mut builder = Command::new(&shell.program);
        for arg in shell.argv {
            builder.arg(arg);
        }

        // Setup child stdin/stdout/stderr as slave fd of PTY.
        // Ownership of fd is transferred to the Stdio structs and will be closed by them at the end of
        // this scope. (It is not an issue that the fd is closed three times since File::drop ignores
        // error on libc::close.).
        builder.stdin(unsafe { Stdio::from_raw_fd(slave) });
        builder.stderr(unsafe { Stdio::from_raw_fd(slave) });
        builder.stdout(unsafe { Stdio::from_raw_fd(slave) });

        // Setup shell environment.
        builder.env("LOGNAME", pw.name);
        builder.env("USER", pw.name);
        builder.env("HOME", pw.dir);

        // Set $SHELL environment variable on macOS, since login does not do it for us.
        #[cfg(target_os = "macos")]
        builder.env(
            "SHELL",
            config
                .shell
                .as_ref()
                .map(|sh| sh.program())
                .unwrap_or(pw.shell),
        );

        unsafe {
            builder.pre_exec(move || {
                // Create a new process group.
                let err = libc::setsid();
                if err == -1 {
                    die!(
                        "Failed to set session id: {}",
                        io::Error::last_os_error()
                    );
                }

                set_controlling_terminal(slave);

                // No longer need slave/master fds.
                libc::close(slave);
                libc::close(master);

                libc::signal(libc::SIGCHLD, libc::SIG_DFL);
                libc::signal(libc::SIGHUP, libc::SIG_DFL);
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                libc::signal(libc::SIGTERM, libc::SIG_DFL);
                libc::signal(libc::SIGALRM, libc::SIG_DFL);

                Ok(())
            });
        }

        // Handle set working directory option.
        // TODO
        // if let Some(dir) = &config.working_directory {
        //     builder.current_dir(dir);
        // }

        // Prepare signal handling before spawning child.
        // TODO
        // let signals = Signals::new(&[sighook::SIGCHLD])
        //     .expect("error preparing signal handling");

        match builder.spawn() {
            Ok(child) => {
                unsafe {
                    // Maybe this should be done outside of this function so nonblocking
                    // isn't forced upon consumers. Although maybe it should be?
                    set_nonblocking(master);
                }

                let mut pty = Pty {
                    child,
                    fd: unsafe { File::from_raw_fd(master) },
                };
                pty.on_resize(&size);
                pty
            }
            Err(err) => {
                die!(
                    "Failed to spawn command '{}': {}",
                    shell.program.display(),
                    err
                )
            }
        }
    }

    /// Resize the PTY.
    ///
    /// Tells the kernel that the window size changed with the new pixel
    /// dimensions and line/column counts.
    pub fn on_resize(&mut self, size: &Size) {
        let win = winsize {
            ws_row: size.rows as _,
            ws_col: size.columns as _,
            ws_xpixel: size.width_in_pixels as _,
            ws_ypixel: size.height_in_pixels as _,
        };

        let res = unsafe {
            libc::ioctl(self.fd.as_raw_fd(), libc::TIOCSWINSZ, &win as *const _)
        };

        if res < 0 {
            die!("ioctl TIOCSWINSZ failed: {}", io::Error::last_os_error());
        }
    }

    pub fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

unsafe fn set_nonblocking(fd: c_int) {
    use libc::{fcntl, F_GETFL, F_SETFL, O_NONBLOCK};

    let res = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL, 0) | O_NONBLOCK);
    assert_eq!(res, 0);
}

#[test]
fn test_get_pw_entry() {
    let mut buf: [i8; 1024] = [0; 1024];
    let _pw = get_pw_entry(&mut buf);
}
