use super::*;
use std::{
    io::Write,
    sync::{atomic::AtomicU64, mpsc},
};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct PrivateDirectory(PathBuf);
impl PrivateDirectory {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let path = std::env::temp_dir().join(format!(
            "rutter-ipc-{}-{}",
            std::process::id(),
            DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct Host {
    server: ActivityServer,
    registry: Arc<Mutex<ActivityRegistry>>,
    full: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    gui: Option<JoinHandle<()>>,
    _directory: PrivateDirectory,
}
impl Host {
    fn new() -> Self {
        let directory = PrivateDirectory::new();
        let (tx, rx) = mpsc::sync_channel::<HostRequest>(8);
        let full = Arc::new(AtomicBool::new(false));
        let gate = full.clone();
        let server = ActivityServer::bind(&directory.0, move |request| {
            if gate.load(Ordering::Acquire) {
                return Err(DeliveryError::Full);
            }
            tx.try_send(request).map_err(|error| match error {
                mpsc::TrySendError::Full(_) => DeliveryError::Full,
                mpsc::TrySendError::Disconnected(_) => DeliveryError::Closed,
            })
        })
        .unwrap();
        let registry = Arc::new(Mutex::new(ActivityRegistry::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let gui_registry = registry.clone();
        let gui_stop = stop.clone();
        let gui = thread::spawn(move || {
            while !gui_stop.load(Ordering::Acquire) {
                if let Ok(request) = rx.recv_timeout(TICK) {
                    let _ = request.apply(&mut gui_registry.lock().unwrap());
                }
            }
        });
        Self {
            server,
            registry,
            full,
            stop,
            gui: Some(gui),
            _directory: directory,
        }
    }
    fn client(&self) -> ActivityClient {
        ActivityClient::connect(self.server.socket_path()).unwrap()
    }
    fn until(&self, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while !condition() {
            assert!(Instant::now() < deadline, "condition timed out");
            thread::sleep(TICK);
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let report = self.server.shutdown();
        assert!(report.endpoint_error.is_none());
        self.stop.store(true, Ordering::Release);
        self.gui.take().unwrap().join().unwrap();
        for cleanup in report.cleanup {
            cleanup.apply(&mut self.registry.lock().unwrap()).unwrap();
        }
    }
}

fn transfer() -> ActivityContent {
    Transfer::new(
        "download",
        TransferDirection::Download,
        2,
        Some(10),
        TransferState::Running,
    )
    .unwrap()
    .into()
}

fn dispatch_ready(
    server: &ActivityServer,
    registry: &ActivityRegistry,
    event: ActivityActionEvent,
) {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match server.dispatch(registry, event) {
            Ok(()) => return,
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(TICK)
            }
            Err(error) => panic!("action dispatch failed: {error}"),
        }
    }
}

#[test]
fn roundtrip_ownership_actions_and_disconnect() {
    let host = Host::new();
    let mut first = host.client();
    let mut second = host.client();
    let id = first.start(transfer()).unwrap();
    assert!(second.replace(id, transfer()).is_err());
    assert!(second.remove(id).is_err());
    assert_eq!(host.registry.lock().unwrap().iter().len(), 1);
    {
        let registry = host.registry.lock().unwrap();
        let activity = registry.iter().next().unwrap();
        let event = registry
            .route_action(activity.id(), ActivityAction::Pause)
            .unwrap();
        dispatch_ready(&host.server, &registry, event);
    }
    assert_eq!(
        first.wait_action(Duration::from_secs(1)).unwrap(),
        Some(ClientAction {
            id,
            action: ActivityAction::Pause
        })
    );
    assert_eq!(second.poll_action().unwrap(), None);
    let media = Media::new(
        "track",
        "artist",
        Duration::from_nanos(42),
        Some(Duration::from_secs(3)),
        MediaState::Playing,
    )
    .unwrap();
    first.replace(id, media.clone()).unwrap();
    assert_eq!(
        host.registry
            .lock()
            .unwrap()
            .iter()
            .next()
            .unwrap()
            .content(),
        &ActivityContent::Media(media)
    );
    first.remove(id).unwrap();
    assert!(first.remove(id).is_err());
    first.start(transfer()).unwrap();
    host.full.store(true, Ordering::Release);
    drop(first);
    thread::sleep(Duration::from_millis(80));
    assert_eq!(host.registry.lock().unwrap().iter().len(), 1);
    host.full.store(false, Ordering::Release);
    host.until(|| host.registry.lock().unwrap().iter().len() == 0);
}

#[test]
fn action_queue_is_bounded_and_replacement_preserves_accepted_actions() {
    let host = Host::new();
    let mut client = host.client();
    let id = client.start(transfer()).unwrap();
    let event = {
        let registry = host.registry.lock().unwrap();
        registry
            .route_action(registry.iter().next().unwrap().id(), ActivityAction::Pause)
            .unwrap()
    };
    for _ in 0..MAX_PENDING_ACTIONS {
        dispatch_ready(&host.server, &host.registry.lock().unwrap(), event);
    }
    assert_eq!(
        host.server
            .dispatch(&host.registry.lock().unwrap(), event)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    client
        .replace(
            id,
            Transfer::new(
                "done",
                TransferDirection::Download,
                10,
                Some(10),
                TransferState::Completed,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(
        host.server
            .dispatch(&host.registry.lock().unwrap(), event)
            .is_err()
    );
    // Even a replacement that no longer supports Pause must not erase requests
    // accepted while the previous snapshot did support it.
    for _ in 0..MAX_PENDING_ACTIONS {
        assert_eq!(
            client.poll_action().unwrap(),
            Some(ClientAction {
                id,
                action: ActivityAction::Pause,
            })
        );
    }
    assert_eq!(client.poll_action().unwrap(), None);
    client.remove(id).unwrap();
    assert!(
        host.server
            .dispatch(&host.registry.lock().unwrap(), event)
            .is_err()
    );
}

#[test]
fn accepted_actions_keep_fifo_order_across_kind_replacement() {
    let host = Host::new();
    let mut client = host.client();
    let id = client.start(transfer()).unwrap();
    for action in [ActivityAction::Pause, ActivityAction::Cancel] {
        let registry = host.registry.lock().unwrap();
        let event = registry
            .route_action(registry.selected().unwrap(), action)
            .unwrap();
        dispatch_ready(&host.server, &registry, event);
    }
    client
        .replace(
            id,
            Media::new("track", "artist", Duration::ZERO, None, MediaState::Playing).unwrap(),
        )
        .unwrap();
    for action in [ActivityAction::Pause, ActivityAction::Cancel] {
        assert_eq!(
            client.poll_action().unwrap(),
            Some(ClientAction { id, action })
        );
    }
    assert_eq!(client.poll_action().unwrap(), None);
}

#[test]
fn disconnected_queued_requests_cannot_allocate_and_cleanup_retries_when_full() {
    let directory = PrivateDirectory::new();
    let (tx, rx) = mpsc::sync_channel(1);
    let full = Arc::new(AtomicBool::new(false));
    let gate = full.clone();
    let mut server = ActivityServer::bind(&directory.0, move |request| {
        if gate.load(Ordering::Acquire) {
            return Err(DeliveryError::Full);
        }
        tx.try_send(request).map_err(|_| DeliveryError::Full)
    })
    .unwrap();
    let mut registry = ActivityRegistry::new();
    for handshake in [true, false] {
        let mut peer = UnixStream::connect(server.socket_path()).unwrap();
        protocol::write(
            &mut peer,
            &Request::Hello {
                version: PROTOCOL_VERSION,
            },
        )
        .unwrap();
        let mut request = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        if !handshake {
            request.apply(&mut registry).unwrap();
            // Applying a clone again must not allocate a second owner.
            request.clone().apply(&mut registry).unwrap();
            assert!(matches!(
                protocol::read::<Reply>(&mut peer).unwrap(),
                Reply::Ready
            ));
            protocol::write(&mut peer, &Request::Start(transfer().into())).unwrap();
            request = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        }
        full.store(true, Ordering::Release);
        drop(peer);
        assert!(request.apply(&mut registry).is_err());
        request.clone().apply(&mut registry).unwrap();
        assert_eq!(registry.iter().len(), 0);
        // Cleanup must remain retained while ingress is full, then be delivered.
        assert!(!request.session.lock().unwrap().cleaned);
        full.store(false, Ordering::Release);
        let cleanup = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        cleanup.apply(&mut registry).unwrap();
        cleanup.clone().apply(&mut registry).unwrap();
        assert!(request.session.lock().unwrap().cleaned);
    }
    let report = server.shutdown();
    assert!(report.endpoint_error.is_none());
    for cleanup in report.cleanup {
        cleanup.apply(&mut registry).unwrap();
    }
    for _ in 0..MAX_OWNERS {
        registry.register_owner().unwrap();
    }
}

#[test]
fn disconnected_peer_cancels_request_while_ingress_is_full() {
    let directory = PrivateDirectory::new();
    let (tx, rx) = mpsc::sync_channel(1);
    let mut server = ActivityServer::bind(&directory.0, move |request| {
        // Observe the attempt without accepting it.
        let _ = tx.try_send(request);
        Err(DeliveryError::Full)
    })
    .unwrap();
    let mut peer = UnixStream::connect(server.socket_path()).unwrap();
    protocol::write(
        &mut peer,
        &Request::Hello {
            version: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    let request = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    drop(peer);
    let deadline = Instant::now() + Duration::from_secs(1);
    while !request.session.lock().unwrap().closed {
        assert!(
            Instant::now() < deadline,
            "disconnect waited for GUI timeout"
        );
        thread::sleep(TICK);
    }
    let mut registry = ActivityRegistry::new();
    assert!(request.apply(&mut registry).is_err());
    let report = server.shutdown();
    assert!(report.endpoint_error.is_none());
    for cleanup in report.cleanup {
        cleanup.apply(&mut registry).unwrap();
    }
    for _ in 0..MAX_OWNERS {
        registry.register_owner().unwrap();
    }
}

#[test]
fn frame_write_to_nonreading_peer_has_a_deadline() {
    let (mut sender, _receiver) = UnixStream::pair().unwrap();
    sender.set_nonblocking(true).unwrap();
    loop {
        match sender.write(&[0]) {
            Ok(count) => assert!(count > 0),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("cannot fill socket: {error}"),
        }
    }
    sender.set_nonblocking(false).unwrap();
    let before = Instant::now();
    let error = protocol::write(&mut sender, &Reply::Ready).unwrap_err();
    assert!(matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    assert!(before.elapsed() < Duration::from_secs(3));
}

#[test]
fn polling_revalidates_actions_after_host_side_removal() {
    let host = Host::new();
    let mut client = host.client();
    client.start(transfer()).unwrap();
    {
        let mut registry = host.registry.lock().unwrap();
        let (id, owner) = {
            let activity = registry.iter().next().unwrap();
            (activity.id(), activity.owner())
        };
        let event = registry.route_action(id, ActivityAction::Pause).unwrap();
        dispatch_ready(&host.server, &registry, event);
        registry.remove(owner, id).unwrap();
    }
    assert_eq!(client.poll_action().unwrap(), None);
}

#[test]
fn saturated_start_is_rejected_and_shutdown_cleans_live_activities() {
    let mut host = Host::new();
    let mut client = host.client();
    client.start(transfer()).unwrap();
    host.full.store(true, Ordering::Release);
    assert!(
        client
            .start(transfer())
            .unwrap_err()
            .to_string()
            .contains("timed out or ingress full")
    );
    assert_eq!(host.registry.lock().unwrap().iter().len(), 1);
    let report = host.server.shutdown();
    assert!(report.endpoint_error.is_none());
    let mut registry = host.registry.lock().unwrap();
    for cleanup in report.cleanup {
        cleanup.apply(&mut registry).unwrap();
    }
    assert_eq!(registry.iter().len(), 0);
    for _ in 0..MAX_OWNERS {
        registry.register_owner().unwrap();
    }
}

#[test]
fn frame_deadline_is_not_reset_by_fragmented_reads() {
    let directory = PrivateDirectory::new();
    let path = directory.0.join("framing.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let mut sender = UnixStream::connect(&path).unwrap();
    let (mut receiver, _) = listener.accept().unwrap();
    let writer = thread::spawn(move || {
        for byte in [0, 0, 0, 1] {
            if sender.write_all(&[byte]).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(700));
        }
    });
    let before = Instant::now();
    let error = protocol::read::<Reply>(&mut receiver).unwrap_err();
    assert!(matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    assert!(before.elapsed() < Duration::from_secs(3));
    drop(receiver);
    writer.join().unwrap();
}

#[test]
fn client_rejects_unexpected_reply_and_poisons_connection() {
    let directory = PrivateDirectory::new();
    let path = directory.0.join("invalid-reply.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        assert!(matches!(
            protocol::read::<Request>(&mut stream).unwrap(),
            Request::Hello { version: 1 }
        ));
        protocol::write(&mut stream, &Reply::Ready).unwrap();
        assert!(matches!(
            protocol::read::<Request>(&mut stream).unwrap(),
            Request::Start(_)
        ));
        protocol::write(&mut stream, &Reply::Accepted).unwrap();
        assert!(protocol::read::<Request>(&mut stream).is_err());
    });
    let mut client = ActivityClient::connect(&path).unwrap();
    assert!(
        client
            .start(transfer())
            .unwrap_err()
            .to_string()
            .contains("unexpected start reply")
    );
    assert!(
        client
            .poll_action()
            .unwrap_err()
            .to_string()
            .contains("unusable")
    );
    peer.join().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn client_connect_fails_boundedly_when_accept_backlog_is_full() {
    use std::os::fd::AsRawFd;
    let directory = PrivateDirectory::new();
    let path = directory.0.join("backlog.sock");
    let listener = UnixListener::bind(&path).unwrap();
    // SAFETY: live listening Unix socket, valid positive backlog.
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 1) }, 0);
    let mut peers = Vec::new();
    let before = Instant::now();
    loop {
        match connect::connect(&path) {
            Ok(peer) => peers.push(peer),
            Err(error) => {
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
                break;
            }
        }
        assert!(peers.len() < 8);
    }
    assert!(before.elapsed() < Duration::from_secs(3));
}

#[test]
fn invalid_frames_versions_and_server_validation() {
    let host = Host::new();
    let mut stream = UnixStream::connect(host.server.socket_path()).unwrap();
    protocol::write(&mut stream, &Request::Hello { version: 2 }).unwrap();
    assert!(matches!(
        protocol::read::<Reply>(&mut stream).unwrap(),
        Reply::Error(_)
    ));
    let mut stream = UnixStream::connect(host.server.socket_path()).unwrap();
    protocol::write(&mut stream, &Request::Hello { version: 1 }).unwrap();
    assert!(matches!(
        protocol::read::<Reply>(&mut stream).unwrap(),
        Reply::Ready
    ));
    let content = protocol::Content::Transfer {
        title: "x".repeat(MAX_TITLE_BYTES + 1),
        supporting: String::new(),
        direction: protocol::Direction::Download,
        completed: 0,
        total: None,
        state: protocol::TransferStatus::Running,
    };
    protocol::write(&mut stream, &Request::Start(content)).unwrap();
    assert!(matches!(
        protocol::read::<Reply>(&mut stream).unwrap(),
        Reply::Error(_)
    ));
    assert_eq!(host.registry.lock().unwrap().iter().len(), 0);
    for bytes in [
        vec![0, 0, 0, 1, b'!'],
        (MAX_FRAME_BYTES as u32 + 1).to_be_bytes().to_vec(),
        vec![0, 0, 0, 0],
    ] {
        let mut peer = UnixStream::connect(host.server.socket_path()).unwrap();
        peer.write_all(&bytes).unwrap();
        peer.shutdown(std::net::Shutdown::Write).unwrap();
        assert!(protocol::read::<Reply>(&mut peer).is_err());
    }
    let mut peer = UnixStream::connect(host.server.socket_path()).unwrap();
    peer.write_all(&[0, 0, 0, 10, b'{']).unwrap();
    peer.shutdown(std::net::Shutdown::Write).unwrap();
    assert!(protocol::read::<Reply>(&mut peer).is_err());
}

#[test]
fn expired_queued_handshake_cannot_allocate_owner() {
    let directory = PrivateDirectory::new();
    let (tx, rx) = mpsc::sync_channel(8);
    let mut server = ActivityServer::bind(&directory.0, move |request| {
        tx.try_send(request).map_err(|_| DeliveryError::Full)
    })
    .unwrap();
    let path = server.socket_path().to_owned();
    let client = thread::spawn(move || ActivityClient::connect(path));
    let request = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(client.join().unwrap().is_err());
    let mut registry = ActivityRegistry::new();
    // The acknowledgement has already timed out; replaying clones is inert.
    request.apply(&mut registry).unwrap();
    request.apply(&mut registry).unwrap();
    for _ in 0..MAX_OWNERS {
        registry.register_owner().unwrap();
    }
    let report = server.shutdown();
    assert!(report.endpoint_error.is_none());
    for cleanup in report.cleanup {
        cleanup.apply(&mut registry).unwrap();
    }
}

#[test]
fn expired_start_and_shutdown_cleanup_with_dead_gui() {
    let directory = PrivateDirectory::new();
    let (tx, rx) = mpsc::sync_channel(8);
    let mut server = ActivityServer::bind(&directory.0, move |request| {
        tx.try_send(request).map_err(|_| DeliveryError::Full)
    })
    .unwrap();
    let path = server.socket_path().to_owned();
    let client = thread::spawn(move || {
        let mut client = ActivityClient::connect(path).unwrap();
        client.start(transfer())
    });
    let mut registry = ActivityRegistry::new();
    rx.recv_timeout(Duration::from_secs(1))
        .unwrap()
        .apply(&mut registry)
        .unwrap();
    let start = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(client.join().unwrap().is_err());
    let before = Instant::now();
    let report = server.shutdown();
    assert!(before.elapsed() < Duration::from_secs(5));
    assert!(report.endpoint_error.is_none());
    for cleanup in report.cleanup {
        cleanup.apply(&mut registry).unwrap();
    }
    start.apply(&mut registry).unwrap();
    assert_eq!(registry.iter().len(), 0);
    for _ in 0..MAX_OWNERS {
        registry.register_owner().unwrap();
    }
}

#[test]
fn directory_permissions_symlinks_existing_endpoints_and_owned_cleanup() {
    use std::os::unix::fs::symlink;
    let directory = PrivateDirectory::new();
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(ActivityServer::bind(&directory.0, |_| Ok(())).is_err());
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let parent = PrivateDirectory::new();
    let link = parent.0.join("link");
    symlink(&directory.0, &link).unwrap();
    assert!(ActivityServer::bind(&link, |_| Ok(())).is_err());
    assert!(ActivityServer::bind(link.join(""), |_| Ok(())).is_err());
    let endpoint = directory.0.join(SOCKET_NAME);
    symlink("missing", &endpoint).unwrap();
    assert!(ActivityServer::bind(&directory.0, |_| Ok(())).is_err());
    assert!(
        fs::symlink_metadata(&endpoint)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    fs::remove_file(&endpoint).unwrap();
    let stale = UnixListener::bind(&endpoint).unwrap();
    drop(stale);
    assert!(ActivityServer::bind(&directory.0, |_| Ok(())).is_err());
    fs::remove_file(&endpoint).unwrap();
    let mut server = ActivityServer::bind(&directory.0, |_| Err(DeliveryError::Closed)).unwrap();
    assert_eq!(fs::metadata(&endpoint).unwrap().mode() & 0o777, 0o600);
    fs::remove_file(&endpoint).unwrap();
    fs::write(&endpoint, b"replacement").unwrap();
    let report = server.shutdown();
    assert!(report.endpoint_error.is_some());
    assert_eq!(fs::read(&endpoint).unwrap(), b"replacement");
}

#[test]
fn connection_limit_and_shutdown_do_not_wait_on_gui() {
    let directory = PrivateDirectory::new();
    let mut server = ActivityServer::bind(&directory.0, |_| Err(DeliveryError::Full)).unwrap();
    let mut peers = Vec::new();
    for _ in 0..MAX_CONNECTIONS {
        peers.push(UnixStream::connect(server.socket_path()).unwrap());
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    while server.sessions.lock().unwrap().len() < MAX_CONNECTIONS {
        assert!(Instant::now() < deadline);
        thread::sleep(TICK);
    }
    let mut ninth = UnixStream::connect(server.socket_path()).unwrap();
    assert!(protocol::read::<Reply>(&mut ninth).is_err());
    let before = Instant::now();
    let report = server.shutdown();
    assert!(before.elapsed() < Duration::from_secs(5));
    assert_eq!(report.cleanup.len(), MAX_CONNECTIONS);
    assert!(report.endpoint_error.is_none());
    assert!(!server.socket_path().exists());
}
