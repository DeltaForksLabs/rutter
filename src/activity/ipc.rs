//! Optional, synchronous Unix activity IPC (`activity-ipc`).
//!
//! This is a **same-UID trust** boundary, not general authentication. Only plain
//! activity snapshots and semantic actions travel over version-1 JSON frames;
//! no paths, artwork, commands or callbacks are transported. The endpoint is
//! `activity.sock` inside an existing, caller-owned mode-0700 directory.
//!
//! `deliver` must be a quick, nonblocking ingress enqueue. Return `Full` to retry
//! or `Closed` when the GUI loop is gone. Apply accepted requests to the same
//! authoritative registry, in FIFO order. Cloned requests apply at most once.
//! A timed-out or disconnected request cannot subsequently create an owner or
//! activity. Disconnect cleanup is retained (and retried on `Full`) while the
//! server runs; undrained sessions count toward the eight-connection limit.
//!
//! **Shutdown obligation:** call [`ActivityServer::shutdown`], then apply every
//! returned cleanup request to the authoritative registry, even if ingress has
//! closed. Already queued requests are safe to drain afterward. Dropping the
//! supervisor stops I/O but cannot mutate a GUI-owned registry: the host must
//! explicitly perform shutdown cleanup before discarding that registry.
//!
//! Frames and GUI acceptance each have a two-second deadline (client reply
//! reads allow three seconds to include GUI scheduling). Idle connections
//! must poll within that interval. Clients serialize all calls; actions arrive
//! only in poll replies. All resource limits are fixed, independent of input.
//! Accepted actions remain FIFO across snapshot replacements, even if the new
//! snapshot no longer supports them. Producers decide how to handle these past
//! requests. Removal or disconnection discards actions for the removed activity;
//! delivery is not acknowledged and cannot be guaranteed across transport failure.
//! The host ingress must itself be bounded. At most one ordinary request per
//! connection awaits GUI acceptance; its reply slot is shared by all clones.
//!
//! Wire framing is a four-byte big-endian length followed by UTF-8 JSON, with
//! an externally tagged operation (for example `{"Hello":{"version":1}}`).
//! Numeric activity handles are only looked up in their originating connection;
//! owner IDs never appear in client requests. Durations retain seconds/nanoseconds.
//!
//! Host integration (the receive loop belongs in the GUI's event ingress):
//! ```no_run
//! # #[cfg(all(unix, feature = "activity-ipc"))]
//! # fn host(private_directory: &std::path::Path) -> std::io::Result<()> {
//! use rutter::activity::{ActivityRegistry, ipc::{ActivityServer, DeliveryError}};
//! use std::sync::mpsc::{sync_channel, TrySendError};
//! let (tx, rx) = sync_channel(32);
//! let mut server = ActivityServer::bind(private_directory, move |request| {
//!     tx.try_send(request).map_err(|error| match error {
//!         TrySendError::Full(_) => DeliveryError::Full,
//!         TrySendError::Disconnected(_) => DeliveryError::Closed,
//!     })
//! })?;
//! let mut registry = ActivityRegistry::new();
//! // In the GUI event loop, apply each received request to this registry.
//! while let Ok(request) = rx.try_recv() {
//!     if let Err(error) = request.apply(&mut registry) {
//!         eprintln!("Activity request rejected: {error}");
//!     }
//! }
//! // Before stopping/discarding GUI state, even if ingress is already closed:
//! let report = server.shutdown();
//! for cleanup in report.cleanup { cleanup.apply(&mut registry)?; }
//! if let Some(error) = report.endpoint_error { return Err(error); }
//! # Ok(()) }
//! ```

use super::*;
use std::{
    collections::{BTreeMap, VecDeque},
    fmt, fs, io,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod client;
mod connect;
mod protocol;
mod server;
pub use client::{ActivityClient, ClientAction, RemoteActivityId};
use protocol::{Reply, Request};
pub use server::{ActivityServer, ShutdownReport};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 16 * 1024;
pub const MAX_CONNECTIONS: usize = 8;
pub const MAX_PENDING_ACTIONS: usize = 32;
pub const IO_TIMEOUT: Duration = Duration::from_secs(2);
pub const SOCKET_NAME: &str = "activity.sock";
const TICK: Duration = Duration::from_millis(10);

/// The ingress callback must not block. A full ingress is retried boundedly for
/// normal requests, and until accepted/shutdown for disconnect cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryError {
    Full,
    Closed,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn unavailable(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, message)
}
fn activity_error(error: ActivityError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[derive(Debug, Default)]
struct SessionState {
    closed: bool,
    cleaned: bool,
    cleanup_queued: bool,
    peer: Option<UnixStream>,
    owner: Option<OwnerId>,
    activities: BTreeMap<u64, ActivityId>,
    actions: VecDeque<protocol::WireAction>,
}
type Session = Arc<Mutex<SessionState>>;
type Deliver = dyn Fn(HostRequest) -> Result<(), DeliveryError> + Send + Sync;

#[derive(Debug)]
struct Ticket {
    deadline: Instant,
    result: Option<Reply>,
}

/// Owned, cloneable GUI ingress event; it contains no registry or socket guard.
/// Only `apply` mutates the GUI registry. Cleanup events are idempotent.
#[derive(Clone)]
pub struct HostRequest {
    session: Session,
    operation: Option<Request>,
    ticket: Arc<Mutex<Ticket>>,
}
impl fmt::Debug for HostRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostRequest")
            .field("cleanup", &self.operation.is_none())
            .finish_non_exhaustive()
    }
}

impl HostRequest {
    fn new(session: &Session, operation: Option<Request>) -> Self {
        Self {
            session: session.clone(),
            operation,
            ticket: Arc::new(Mutex::new(Ticket {
                deadline: Instant::now() + IO_TIMEOUT,
                result: None,
            })),
        }
    }

    /// Apply once to the authoritative GUI registry and acknowledge acceptance.
    /// Expired/cancelled operations fail without mutation; cleanup never expires.
    pub fn apply(&self, registry: &mut ActivityRegistry) -> io::Result<()> {
        let mut state = self.session.lock().unwrap();
        if self.operation.is_none() {
            state.closed = true;
            if let Some(owner) = state.owner.take() {
                // An external host removal is already-clean state.
                match registry.remove_owner(owner) {
                    Ok(_) | Err(ActivityError::UnknownOwner(_)) => {}
                    Err(error) => return Err(activity_error(error)),
                }
            }
            state.activities.clear();
            state.actions.clear();
            state.peer = None;
            state.cleaned = true;
            return Ok(());
        }
        let mut ticket = self.ticket.lock().unwrap();
        if ticket.result.is_some() {
            return Ok(());
        }
        // Check at acceptance too: the worker may not have observed EOF yet.
        if let Some(peer) = &state.peer {
            match server::peer_disconnected(peer) {
                Ok(true) => state.closed = true,
                Ok(false) => {}
                Err(error) => {
                    state.closed = true;
                    return Err(error);
                }
            }
        }
        if state.closed || Instant::now() >= ticket.deadline {
            ticket.result = Some(Reply::Error(
                "GUI request expired or connection closed".into(),
            ));
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "GUI request expired or connection closed",
            ));
        }
        let result = apply_operation(&mut state, registry, self.operation.as_ref().unwrap());
        ticket.result = Some(match &result {
            Ok(reply) => reply.clone(),
            Err(error) => Reply::Error(error.to_string()),
        });
        result.map(|_| ())
    }
}

fn apply_operation(
    state: &mut SessionState,
    registry: &mut ActivityRegistry,
    request: &Request,
) -> io::Result<Reply> {
    if let Request::Hello {
        version: PROTOCOL_VERSION,
    } = request
    {
        if state.owner.is_some() {
            return Err(invalid("duplicate handshake"));
        }
        state.owner = Some(registry.register_owner().map_err(activity_error)?);
        return Ok(Reply::Ready);
    }
    let owner = state
        .owner
        .ok_or_else(|| invalid("version-1 handshake required"))?;
    // Hosts may remove activities directly. Do not retain stale mappings or
    // allow their accumulation across subsequent starts.
    state.activities.retain(|_, id| {
        registry
            .get(*id)
            .is_some_and(|activity| activity.owner() == owner)
    });
    match request {
        Request::Start(content) => {
            let content = content.clone().validate().map_err(activity_error)?;
            let id = registry.start(owner, content).map_err(activity_error)?;
            state.activities.insert(id.0, id);
            Ok(Reply::Started(id.0))
        }
        Request::Replace { id, content } => {
            let activity = *state
                .activities
                .get(id)
                .ok_or_else(|| invalid("unknown connection activity"))?;
            registry
                .replace(
                    owner,
                    activity,
                    content.clone().validate().map_err(activity_error)?,
                )
                .map_err(activity_error)?;
            Ok(Reply::Accepted)
        }
        Request::Remove { id } => {
            let activity = *state
                .activities
                .get(id)
                .ok_or_else(|| invalid("unknown connection activity"))?;
            registry.remove(owner, activity).map_err(activity_error)?;
            state.activities.remove(id);
            state.actions.retain(|action| action.id != *id);
            Ok(Reply::Accepted)
        }
        Request::Poll => {
            while let Some(action) = state.actions.pop_front() {
                if let Some(id) = state.activities.get(&action.id)
                    && registry
                        .get(*id)
                        .is_some_and(|activity| activity.owner() == owner)
                {
                    return Ok(Reply::Action(Some(action)));
                }
            }
            Ok(Reply::Action(None))
        }
        _ => Err(invalid("invalid GUI operation")),
    }
}

#[cfg(test)]
mod tests;
