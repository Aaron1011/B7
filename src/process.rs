use binary::Binary;
use libc;
use libc::{c_int, c_void};
use nix;
use nix::sys::ptrace;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::Pid;
use spawn_ptrace::CommandPtraceSpawn;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::os::unix::io::FromRawFd;
use std::process::{Child, Command, Stdio};

use b7777::get_perf_fd;

#[derive(Debug)]
pub struct Process {
    binary: Binary,
    cmd: Command,
    child: Option<Child>,
    perf_fd: c_int,
}

// Handle running a process
impl Process {
    pub fn new(path: &str) -> Process {
        Process {
            binary: Binary::new(path),
            cmd: Command::new(path),
            child: None,
            perf_fd: -1,
        }
    }

    pub fn args<I, S>(&mut self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.cmd.args(args);
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) {
        self.cmd.arg(arg);
    }

    // initialize process and wait it
    pub fn start(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Err(Error::new(
                ErrorKind::Other,
                "child process already running",
            ));
        }
        self.cmd.stdin(Stdio::piped());
        self.cmd.stdout(Stdio::piped());
        self.cmd.stderr(Stdio::piped());
        // spawn process and wait after fork
        let child = self.cmd.spawn_ptrace();
        match child {
            Ok(c) => {
                self.child = Some(c);
                Ok(())
            }
            Err(c) => Err(c),
        }
    }

    // write buf to process then close it
    pub fn write_stdin(&mut self, buf: &[u8]) -> Result<()> {
        if self.child.is_none() {
            return Err(Error::new(ErrorKind::Other, "child process not running"));
        }
        let child = self.child.as_mut().unwrap();
        match child.stdin.as_mut() {
            Some(stdin) => stdin.write_all(buf),
            None => Err(Error::last_os_error()),
        }
    }

    // read buf to process then close it
    pub fn read_stdout(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        if self.child.is_none() {
            return Err(Error::new(ErrorKind::Other, "child process not running"));
        }
        let child = self.child.as_mut().unwrap();
        match child.stdout.as_mut() {
            Some(stdout) => stdout.read_to_end(buf),
            None => Err(Error::last_os_error()),
        }
    }

    // close stdin to prevent any reads hanging
    pub fn close_stdin(&mut self) -> Result<()> {
        if self.child.is_none() {
            return Err(Error::new(ErrorKind::Other, "child process not running"));
        }
        match self.child.as_mut().unwrap().stdin.take() {
            Some(stdin) => {
                drop(stdin);
                Ok(())
            }
            None => Err(Error::last_os_error()),
        }
    }

    // initalize perf at current point in execution
    pub fn init_perf(&mut self) -> Result<()> {
        match self.child {
            None => Err(Error::new(ErrorKind::Other, "child process not running")),
            Some(ref child) => {
                self.perf_fd = get_perf_fd(child.id() as i32);
                Ok(())
            }
        }
    }

    // clean up the perf file descriptor
    pub fn close_perf(&mut self) {
        unsafe {
            drop(File::from_raw_fd(self.perf_fd));
        }
        self.perf_fd = -1;
    }

    // continue executing ptrace if it is paused
    pub fn cont(&self) -> Result<()> {
        if self.child.is_none() {
            return Err(Error::new(ErrorKind::Other, "child process not running"));
        }
        let child = self.child.as_ref().unwrap();
        let res = ptrace::cont(Pid::from_raw(child.id() as i32), None);
        match res {
            Ok(_) => Ok(()),
            Err(x) => Err(Error::new(ErrorKind::Other, format!("{:?}", x))),
        }
    }

    // go until next pause point
    pub fn wait(&self) -> Result<WaitStatus> {
        if self.child.is_none() {
            return Err(Error::new(ErrorKind::Other, "child process not running"));
        }
        let child = self.child.as_ref().unwrap();
        match waitpid(Pid::from_raw(child.id() as i32), None) {
            Err(x) => Err(Error::new(ErrorKind::Other, format!("{:?}", x))),
            Ok(x) => Ok(x),
        }
    }

    // attempt to run the program to completion
    pub fn finish(&self) -> Result<()> {
        loop {
            let cret = self.cont();
            if cret.is_err() {
                return cret;
            }
            let wret = self.wait();
            match wret {
                Ok(WaitStatus::Exited(_, _)) => return Ok(()),
                Err(x) => return Err(x),
                _ => (),
            }
        }
    }

    // read the instruction count stoed if perf is establised
    pub fn get_inst_count(&self) -> Result<i64> {
        let mut count: i64 = 0;
        let count_p = &mut count as *mut i64;
        let nread: i64;
        unsafe {
            nread = libc::read(self.perf_fd, count_p as *mut c_void, 8) as i64;
        };
        match nread {
            8 => Ok(count),
            x if x >= 0 => Err(Error::new(ErrorKind::Other, nread.to_string())),
            _ => Err(Error::new(ErrorKind::Other, nix::Error::last())),
        }
    }
}
