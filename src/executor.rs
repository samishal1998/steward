//! Bounded native argv execution. No shell interpolation or unbounded output capture.
use crate::error::{Diagnostic, Result};
use std::{
    ffi::OsStr,
    io::{Read, Seek, SeekFrom},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn run(command: &mut Command, timeout: Duration, limit: u64) -> Result<Output> {
    let mut out = tempfile::tempfile().map_err(io_error)?;
    let mut err = tempfile::tempfile().map_err(io_error)?;
    command
        .stdin(Stdio::null())
        .stdout(out.try_clone().map_err(io_error)?)
        .stderr(err.try_clone().map_err(io_error)?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|_| {
        Diagnostic::new(
            "process.start",
            "Could not start the selected native executable",
        )
    })?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        let large = out.metadata().map_err(io_error)?.len()
            + err.metadata().map_err(io_error)?.len()
            > limit;
        if large || Instant::now() > deadline {
            #[cfg(unix)]
            // SAFETY: the child was created as leader of its own process group.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(Diagnostic::new(
                if large {
                    "process.output_limit"
                } else {
                    "process.timeout"
                },
                "Native command exceeded its bounded execution budget",
            ));
        }
        if let Some(status) = child.try_wait().map_err(io_error)? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    out.seek(SeekFrom::Start(0)).map_err(io_error)?;
    err.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    out.take(limit + 1)
        .read_to_end(&mut stdout)
        .map_err(io_error)?;
    err.take(limit + 1)
        .read_to_end(&mut stderr)
        .map_err(io_error)?;
    if (stdout.len() + stderr.len()) as u64 > limit {
        return Err(Diagnostic::new(
            "process.output_limit",
            "Native output exceeds its budget",
        ));
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

pub fn checked(program: impl AsRef<OsStr>, args: &[&OsStr]) -> Result<Vec<u8>> {
    let output = run(
        Command::new(program).args(args),
        Duration::from_secs(30),
        1024 * 1024,
    )?;
    if !output.status.success() {
        return Err(Diagnostic::new(
            "process.failed",
            format!(
                "Native command failed with status {} ({} bytes of private diagnostics); inspect the service/proxy logs",
                output.status,
                output.stderr.len()
            ),
        ));
    }
    Ok(output.stdout)
}

fn io_error(_: std::io::Error) -> Diagnostic {
    Diagnostic::new("process.io", "Native process output could not be accessed")
}
