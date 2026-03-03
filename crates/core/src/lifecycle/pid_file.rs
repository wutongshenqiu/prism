//! RAII PID file management with advisory file locking.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// RAII guard for a PID file. Acquires an exclusive advisory lock on creation
/// and removes the file on drop.
pub struct PidFile {
    path: PathBuf,
    fd: std::os::unix::io::RawFd,
}

impl PidFile {
    /// Acquire a PID file at `path`. Writes the current PID and holds an
    /// exclusive `flock`. Returns an error if another process holds the lock.
    pub fn acquire(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        use std::os::unix::io::AsRawFd;

        let path = path.as_ref().to_path_buf();

        // Create/open the file
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        let fd = file.as_raw_fd();

        // Try exclusive non-blocking lock
        let ret = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!(
                "Failed to lock PID file {}: {} (another instance running?)",
                path.display(),
                err
            );
        }

        // Write PID — keep fd open (flock is tied to fd lifetime)
        let pid = std::process::id();
        let mut file = file;
        write!(file, "{}", pid)?;
        file.flush()?;

        // Leak the File so fd stays open; we manage cleanup in Drop
        std::mem::forget(file);

        Ok(Self { path, fd })
    }

    /// Read the PID stored in a PID file.
    pub fn read_pid(path: impl AsRef<Path>) -> anyhow::Result<u32> {
        let contents = fs::read_to_string(path.as_ref())?;
        let pid: u32 = contents.trim().parse()?;
        Ok(pid)
    }

    /// Check whether a process with the given PID is alive.
    pub fn is_alive(pid: u32) -> bool {
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    /// Send a signal to a process.
    pub fn send_signal(pid: u32, signal: i32) -> anyhow::Result<()> {
        let ret = unsafe { libc::kill(pid as libc::pid_t, signal) };
        if ret != 0 {
            anyhow::bail!(
                "Failed to send signal {} to PID {}: {}",
                signal,
                pid,
                std::io::Error::last_os_error()
            );
        }
        Ok(())
    }

    /// Gracefully stop a process: send SIGTERM, poll for exit up to `timeout`,
    /// then SIGKILL as fallback.
    pub fn stop(pid: u32, timeout: std::time::Duration) -> anyhow::Result<()> {
        if !Self::is_alive(pid) {
            return Ok(());
        }

        // Send SIGTERM
        Self::send_signal(pid, libc::SIGTERM)?;

        // Poll for exit
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(100);
        while start.elapsed() < timeout {
            if !Self::is_alive(pid) {
                return Ok(());
            }
            std::thread::sleep(poll_interval);
        }

        // Fallback: SIGKILL
        if Self::is_alive(pid) {
            tracing::warn!("PID {} did not exit within timeout, sending SIGKILL", pid);
            Self::send_signal(pid, libc::SIGKILL)?;
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        Ok(())
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        // flock is released when fd is closed
        unsafe {
            libc::close(self.fd);
        }
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_acquire_and_drop_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("test.pid");

        {
            let _pf = PidFile::acquire(&pid_path).unwrap();
            assert!(pid_path.exists());

            // Verify PID content
            let mut content = String::new();
            fs::File::open(&pid_path)
                .unwrap()
                .read_to_string(&mut content)
                .unwrap();
            let pid: u32 = content.trim().parse().unwrap();
            assert_eq!(pid, std::process::id());
        }

        // After drop, file should be removed
        assert!(!pid_path.exists());
    }

    #[test]
    fn test_flock_contention() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("test_contention.pid");

        let _pf = PidFile::acquire(&pid_path).unwrap();
        // Second acquire should fail because lock is held
        let result = PidFile::acquire(&pid_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_alive() {
        // Current process should be alive
        assert!(PidFile::is_alive(std::process::id()));
        // PID 0 (kernel) — kill(0,0) checks the calling process's group;
        // use a very large PID that almost certainly doesn't exist
        assert!(!PidFile::is_alive(u32::MAX - 1));
    }

    #[test]
    fn test_read_pid_missing_file() {
        let result = PidFile::read_pid("/tmp/nonexistent_prism_test.pid");
        assert!(result.is_err());
    }
}
