use super::*;

/// Connection-local remote handle. Sending a handle from another connection
/// is rejected by the server, even when that other activity is still live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteActivityId(pub(super) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientAction {
    pub id: RemoteActivityId,
    pub action: ActivityAction,
}

/// Synchronous client. Any transport/protocol error poisons the connection;
/// reconnect explicitly. A remote validation rejection does not poison it.
#[derive(Debug)]
pub struct ActivityClient {
    stream: UnixStream,
    failed: bool,
}

impl ActivityClient {
    /// Connect to an explicit endpoint and complete the version-1 handshake.
    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let stream = connect::connect(path.as_ref())?;
        stream.set_read_timeout(Some(IO_TIMEOUT + Duration::from_secs(1)))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;
        let mut client = Self {
            stream,
            failed: false,
        };
        match client.exchange(Request::Hello {
            version: PROTOCOL_VERSION,
        })? {
            Reply::Ready => Ok(client),
            _ => Err(invalid("unexpected handshake reply")),
        }
    }

    pub fn start(&mut self, content: impl Into<ActivityContent>) -> io::Result<RemoteActivityId> {
        match self.exchange(Request::Start(content.into().into()))? {
            Reply::Started(id) => Ok(RemoteActivityId(id)),
            _ => Err(self.unexpected("unexpected start reply")),
        }
    }
    pub fn replace(
        &mut self,
        id: RemoteActivityId,
        content: impl Into<ActivityContent>,
    ) -> io::Result<()> {
        match self.exchange(Request::Replace {
            id: id.0,
            content: content.into().into(),
        })? {
            Reply::Accepted => Ok(()),
            _ => Err(self.unexpected("unexpected replace reply")),
        }
    }
    pub fn remove(&mut self, id: RemoteActivityId) -> io::Result<()> {
        match self.exchange(Request::Remove { id: id.0 })? {
            Reply::Accepted => Ok(()),
            _ => Err(self.unexpected("unexpected remove reply")),
        }
    }
    /// One synchronous poll, without waiting for a future action.
    pub fn poll_action(&mut self) -> io::Result<Option<ClientAction>> {
        match self.exchange(Request::Poll)? {
            Reply::Action(action) => Ok(action.map(|a| ClientAction {
                id: RemoteActivityId(a.id),
                action: a.action.into(),
            })),
            _ => Err(self.unexpected("unexpected action reply")),
        }
    }
    /// Poll until an action arrives or the caller's timeout elapses. Each poll
    /// also refreshes the connection's idle deadline. The timeout is checked
    /// between exchanges; an in-flight exchange can take up to five seconds.
    pub fn wait_action(&mut self, timeout: Duration) -> io::Result<Option<ClientAction>> {
        let start = Instant::now();
        loop {
            if let Some(action) = self.poll_action()? {
                return Ok(Some(action));
            }
            if start.elapsed() >= timeout {
                return Ok(None);
            }
            thread::sleep(TICK.min(timeout.saturating_sub(start.elapsed())));
        }
    }
    fn unexpected(&mut self, message: &str) -> io::Error {
        self.failed = true;
        // Preserve the protocol error even if shutting down the failed socket
        // also reports an error; no subsequent call will use this descriptor.
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        invalid(message)
    }

    fn exchange(&mut self, request: Request) -> io::Result<Reply> {
        if self.failed {
            return Err(invalid("IPC connection is unusable; reconnect"));
        }
        let reply = protocol::write(&mut self.stream, &request).and_then(|_| {
            protocol::read_timeout(&mut self.stream, IO_TIMEOUT + Duration::from_secs(1))
        });
        match reply {
            Ok(Reply::Error(message)) => Err(invalid(format!(
                "activity host rejected request: {message}"
            ))),
            Ok(reply) => Ok(reply),
            Err(error) => {
                self.failed = true;
                let _ = self.stream.shutdown(std::net::Shutdown::Both);
                Err(error)
            }
        }
    }
}
