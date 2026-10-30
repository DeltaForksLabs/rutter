use super::*;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;

/// I/O supervisor, deliberately separate from cloneable GUI state.
pub struct ActivityServer {
    path: PathBuf,
    identity: Option<(u64, u64)>,
    stop: Arc<AtomicBool>,
    pub(super) sessions: Arc<Mutex<Vec<Session>>>,
    supervisor: Option<JoinHandle<()>>,
}

/// Cleanup is required even if unlinking the socket failed.
#[derive(Debug)]
#[must_use = "apply cleanup to the authoritative registry and inspect endpoint_error"]
pub struct ShutdownReport {
    pub cleanup: Vec<HostRequest>,
    pub endpoint_error: Option<io::Error>,
}

impl ActivityServer {
    /// Bind `activity.sock` without replacing any existing endpoint. The
    /// directory must already exist, not be a symlink, and be owned by the
    /// effective UID with exactly mode 0700. Ancestors are trusted host input.
    /// The callback must return promptly; blocking callbacks void shutdown's
    /// bound. It must enqueue in FIFO order and never apply inline on a worker.
    pub fn bind(
        private_directory: impl AsRef<Path>,
        deliver: impl Fn(HostRequest) -> Result<(), DeliveryError> + Send + Sync + 'static,
    ) -> io::Result<Self> {
        // Normalize trailing slashes/dots so they cannot mask a final symlink.
        let directory: PathBuf = private_directory.as_ref().components().collect();
        let metadata = fs::symlink_metadata(&directory)?;
        // SAFETY: geteuid takes no pointers and has no preconditions.
        let uid = unsafe { libc::geteuid() };
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "IPC directory must be a real, effective-UID-owned mode-0700 directory",
            ));
        }
        // Keep cleanup independent of later working-directory changes.
        let path = fs::canonicalize(&directory)?.join(SOCKET_NAME);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "IPC endpoint already exists",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(&path)?;
        let metadata = fs::symlink_metadata(&path)?;
        let identity = (metadata.dev(), metadata.ino());
        let stop = Arc::new(AtomicBool::new(false));
        let sessions = Arc::new(Mutex::new(Vec::new()));
        let mut server = Self {
            path,
            identity: Some(identity),
            stop,
            sessions,
            supervisor: None,
        };
        fs::set_permissions(&server.path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let stop = server.stop.clone();
        let sessions = server.sessions.clone();
        let deliver: Arc<Deliver> = Arc::new(deliver);
        server.supervisor = Some(
            thread::Builder::new()
                .name("activity-ipc".into())
                .spawn(move || supervise(listener, stop, sessions, deliver))?,
        );
        Ok(server)
    }

    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    /// Nonblocking GUI action enqueue. Validate against the live authoritative
    /// registry and connection ownership; full/contended queues return WouldBlock.
    /// Accepted requests survive replacement in FIFO order; removal discards
    /// them. Success means queued, not acknowledged by the producer.
    pub fn dispatch(
        &self,
        registry: &ActivityRegistry,
        event: ActivityActionEvent,
    ) -> io::Result<()> {
        if self.stop.load(Ordering::Acquire) {
            return Err(invalid("IPC server stopped"));
        }
        let current = registry
            .route_action(event.id(), event.action())
            .map_err(activity_error)?;
        if current.owner() != event.owner() {
            return Err(invalid("stale action owner"));
        }
        let sessions = self
            .sessions
            .try_lock()
            .map_err(|_| unavailable("IPC sessions busy"))?;
        for session in sessions.iter() {
            let mut state = session
                .try_lock()
                .map_err(|_| unavailable("IPC session busy"))?;
            if !state.closed
                && state.owner == Some(event.owner())
                && state.activities.get(&event.id().0) == Some(&event.id())
            {
                if state.actions.len() == MAX_PENDING_ACTIONS {
                    return Err(unavailable("IPC action queue full"));
                }
                state.actions.push_back(protocol::WireAction {
                    id: event.id().0,
                    action: event.action().into(),
                });
                return Ok(());
            }
        }
        Err(invalid("action has no live IPC owner"))
    }

    /// Stop accepting, cancel sessions, join I/O threads, and unlink only the
    /// owned socket inode. With a nonblocking callback this takes at most one
    /// read and write deadline plus scheduling overhead, regardless of GUI availability.
    /// Apply ALL returned cleanup requests before continuing to use the GUI registry.
    #[must_use = "apply returned cleanup requests to the authoritative registry"]
    pub fn shutdown(&mut self) -> ShutdownReport {
        self.stop.store(true, Ordering::Release);
        for session in self.sessions.lock().unwrap().iter() {
            session.lock().unwrap().closed = true;
        }
        if let Some(supervisor) = self.supervisor.take() {
            supervisor.join().expect("activity IPC supervisor panicked");
        }
        let endpoint_error = self.unlink_owned().err();
        let cleanup = self
            .sessions
            .lock()
            .unwrap()
            .drain(..)
            .map(|session| HostRequest::new(&session, None))
            .collect();
        ShutdownReport {
            cleanup,
            endpoint_error,
        }
    }
    fn unlink_owned(&mut self) -> io::Result<()> {
        let Some(identity) = self.identity else {
            return Ok(());
        };
        match fs::symlink_metadata(&self.path) {
            Ok(metadata)
                if metadata.file_type().is_socket()
                    && (metadata.dev(), metadata.ino()) == identity =>
            {
                fs::remove_file(&self.path)?;
                self.identity = None;
                Ok(())
            }
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "IPC endpoint replaced; refusing to unlink",
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.identity = None;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}
impl Drop for ActivityServer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn supervise(
    listener: UnixListener,
    stop: Arc<AtomicBool>,
    sessions: Arc<Mutex<Vec<Session>>>,
    deliver: Arc<Deliver>,
) {
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !stop.load(Ordering::Acquire) {
        workers = workers
            .into_iter()
            .filter_map(|worker| {
                if worker.is_finished() {
                    worker.join().expect("activity IPC worker panicked");
                    None
                } else {
                    Some(worker)
                }
            })
            .collect();
        {
            let mut sessions = sessions.lock().unwrap();
            sessions.retain(|session| !session.lock().unwrap().cleaned);
            for session in sessions.iter() {
                retry_cleanup(session, &deliver);
            }
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let mut sessions = sessions.lock().unwrap();
                if sessions.len() >= MAX_CONNECTIONS || workers.len() >= MAX_CONNECTIONS {
                    // Do not write to an untrusted non-reading peer here.
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                let session = Arc::new(Mutex::new(SessionState::default()));
                sessions.push(session.clone());
                let deliver = deliver.clone();
                let stop = stop.clone();
                match thread::Builder::new()
                    .name("activity-ipc-session".into())
                    .spawn({
                        let session = session.clone();
                        move || {
                            let _ = serve(stream, &session, &deliver, &stop);
                            session.lock().unwrap().closed = true;
                        }
                    }) {
                    Ok(worker) => workers.push(worker),
                    Err(_) => session.lock().unwrap().closed = true,
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(TICK),
            Err(_) => {
                stop.store(true, Ordering::Release);
            }
        }
    }
    for session in sessions.lock().unwrap().iter() {
        session.lock().unwrap().closed = true;
    }
    for worker in workers {
        worker.join().expect("activity IPC worker panicked");
    }
}

fn retry_cleanup(session: &Session, deliver: &Arc<Deliver>) {
    let needed = {
        let state = session.lock().unwrap();
        state.closed && !state.cleaned && !state.cleanup_queued
    };
    if needed && deliver(HostRequest::new(session, None)).is_ok() {
        session.lock().unwrap().cleanup_queued = true;
    }
}

fn serve(
    mut stream: UnixStream,
    session: &Session,
    deliver: &Arc<Deliver>,
    stop: &AtomicBool,
) -> io::Result<()> {
    session.lock().unwrap().peer = Some(stream.try_clone()?);
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let hello: Request = protocol::read(&mut stream)?;
    if !matches!(
        hello,
        Request::Hello {
            version: PROTOCOL_VERSION
        }
    ) {
        protocol::write(
            &mut stream,
            &Reply::Error("version-1 handshake required".into()),
        )?;
        return Err(invalid("unsupported IPC handshake"));
    }
    let reply = gui_request(session, hello, deliver, stop)?;
    protocol::write(&mut stream, &reply)?;
    if !matches!(reply, Reply::Ready) {
        return Ok(());
    }
    while !stop.load(Ordering::Acquire) {
        let request = match protocol::read::<Request>(&mut stream) {
            Ok(request) => request,
            Err(error) => {
                let _ = protocol::write(&mut stream, &Reply::Error(error.to_string()));
                return Err(error);
            }
        };
        let reply = if matches!(request, Request::Hello { .. }) {
            Reply::Error("duplicate handshake".into())
        } else {
            gui_request(session, request, deliver, stop)?
        };
        protocol::write(&mut stream, &reply)?;
    }
    Ok(())
}

fn gui_request(
    session: &Session,
    request: Request,
    deliver: &Arc<Deliver>,
    stop: &AtomicBool,
) -> io::Result<Reply> {
    let request = HostRequest::new(session, Some(request));
    let mut queued = false;
    loop {
        if stop.load(Ordering::Acquire) {
            return Err(invalid("IPC server stopped"));
        }
        {
            let mut state = session.lock().unwrap();
            if state.closed
                || state
                    .peer
                    .as_ref()
                    .map(peer_disconnected)
                    .transpose()?
                    .unwrap_or(false)
            {
                state.closed = true;
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "IPC peer disconnected",
                ));
            }
        }
        {
            let mut ticket = request.ticket.lock().unwrap();
            if let Some(reply) = ticket.result.clone() {
                return Ok(reply);
            }
            if Instant::now() >= ticket.deadline {
                ticket.result = Some(Reply::Error(
                    "GUI acceptance timed out or ingress full".into(),
                ));
                return Ok(ticket.result.clone().unwrap());
            }
        }
        if !queued {
            match deliver(request.clone()) {
                Ok(()) => queued = true,
                Err(DeliveryError::Full) => {}
                Err(DeliveryError::Closed) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "GUI ingress closed",
                    ));
                }
            }
        }
        thread::sleep(TICK);
    }
}

// Unlike a blocking read, this also works while ingress is full or a request
// awaits acceptance. Peek leaves any pipelined frame bytes untouched.
pub(super) fn peer_disconnected(stream: &UnixStream) -> io::Result<bool> {
    let mut descriptor = libc::pollfd {
        fd: stream.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: descriptor points to one live pollfd; zero timeout never blocks.
    if unsafe { libc::poll(&mut descriptor, 1, 0) } < 0 {
        let error = io::Error::last_os_error();
        return if error.kind() == io::ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(error)
        };
    }
    if descriptor.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
        return Ok(true);
    }
    if descriptor.revents & libc::POLLIN == 0 {
        return Ok(false);
    }
    let mut byte = 0u8;
    // SAFETY: byte is writable for one byte and the socket is live. DONTWAIT
    // makes this bounded without changing the shared descriptor's I/O mode.
    let count = unsafe {
        libc::recv(
            stream.as_raw_fd(),
            (&mut byte as *mut u8).cast(),
            1,
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    if count >= 0 {
        return Ok(count == 0);
    }
    let error = io::Error::last_os_error();
    match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted => Ok(false),
        io::ErrorKind::ConnectionReset | io::ErrorKind::NotConnected => Ok(true),
        _ => Err(error),
    }
}
