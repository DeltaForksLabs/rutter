//! Bounded connect: std's blocking UnixStream::connect can wait indefinitely
//! when an endpoint's accept backlog is full.
use super::*;
use std::os::{
    fd::{AsRawFd, FromRawFd, OwnedFd},
    unix::ffi::OsStrExt,
};

pub(super) fn connect(path: &Path) -> io::Result<UnixStream> {
    // SAFETY: Zero is valid for sockaddr_un's integer and byte-array fields.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "IPC socket path is empty, contains NUL, or exceeds the Unix socket path limit",
        ));
    }
    address.sun_family = libc::AF_UNIX as _;
    for (target, byte) in address.sun_path.iter_mut().zip(bytes) {
        *target = *byte as _;
    }
    let length =
        (std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1) as libc::socklen_t;
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        address.sun_len = length as _;
    }
    // SAFETY: socket has no pointer arguments; a successful fd is immediately owned.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: raw is a fresh, uniquely owned descriptor.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: fcntl is called on a live descriptor with valid flag arguments.
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let stream = UnixStream::from(fd);
    stream.set_nonblocking(true)?;
    // SAFETY: address is initialized and length covers only its path and header.
    let result = unsafe {
        libc::connect(
            stream.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            length,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS)
            && error.kind() != io::ErrorKind::Interrupted
        {
            // In particular, a saturated Unix backlog may return EAGAIN. Fail
            // immediately rather than creating an unbounded connect retry loop.
            return Err(error);
        }
        wait_connected(&stream)?;
    }
    stream.set_nonblocking(false)?;
    Ok(stream)
}

fn wait_connected(stream: &UnixStream) -> io::Result<()> {
    let deadline = Instant::now() + IO_TIMEOUT;
    loop {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "IPC connect deadline elapsed")
            })?;
        let mut descriptor = libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: descriptor points to one live pollfd; timeout is bounded.
        let result =
            unsafe { libc::poll(&mut descriptor, 1, left.as_millis().max(1) as libc::c_int) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if result == 0 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "IPC connect deadline elapsed",
            ));
        }
        if let Some(error) = stream.take_error()? {
            return Err(error);
        }
        stream.peer_addr()?;
        return Ok(());
    }
}
