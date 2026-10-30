use super::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::io::{Read, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum Request {
    Hello { version: u32 },
    Start(Content),
    Replace { id: u64, content: Content },
    Remove { id: u64 },
    Poll,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum Reply {
    Ready,
    Started(u64),
    Accepted,
    Action(Option<WireAction>),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WireAction {
    pub id: u64,
    pub action: Action,
}

macro_rules! wire_enum {
    ($wire:ident, $core:ident, $($variant:ident),+) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
        pub(super) enum $wire { $($variant),+ }
        impl From<$core> for $wire {
            fn from(value: $core) -> Self { match value { $($core::$variant => Self::$variant),+ } }
        }
        impl From<$wire> for $core {
            fn from(value: $wire) -> Self { match value { $($wire::$variant => Self::$variant),+ } }
        }
    };
}
wire_enum!(Direction, TransferDirection, Download, Upload);
wire_enum!(TransferStatus, TransferState, Running, Paused, Completed);
wire_enum!(MediaStatus, MediaState, Playing, Paused, Stopped);
wire_enum!(
    Action,
    ActivityAction,
    Pause,
    Resume,
    Cancel,
    Previous,
    Next
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum Content {
    Transfer {
        title: String,
        supporting: String,
        direction: Direction,
        completed: u64,
        total: Option<u64>,
        state: TransferStatus,
    },
    Media {
        title: String,
        artist: String,
        position: Duration,
        duration: Option<Duration>,
        state: MediaStatus,
    },
}

impl From<ActivityContent> for Content {
    fn from(content: ActivityContent) -> Self {
        match content {
            ActivityContent::Transfer(t) => Self::Transfer {
                title: t.title().into(),
                supporting: t.supporting().into(),
                direction: t.direction().into(),
                completed: t.completed(),
                total: t.total(),
                state: t.state().into(),
            },
            ActivityContent::Media(m) => Self::Media {
                title: m.title().into(),
                artist: m.artist().into(),
                position: m.position(),
                duration: m.duration(),
                state: m.state().into(),
            },
        }
    }
}

impl Content {
    pub fn validate(self) -> Result<ActivityContent, ActivityError> {
        match self {
            Self::Transfer {
                title,
                supporting,
                direction,
                completed,
                total,
                state,
            } => Ok(
                Transfer::new(title, direction.into(), completed, total, state.into())?
                    .with_supporting(supporting)?
                    .into(),
            ),
            Self::Media {
                title,
                artist,
                position,
                duration,
                state,
            } => Ok(Media::new(title, artist, position, duration, state.into())?.into()),
        }
    }
}

pub(super) fn read<T: DeserializeOwned>(stream: &mut UnixStream) -> io::Result<T> {
    read_timeout(stream, IO_TIMEOUT)
}

pub(super) fn read_timeout<T: DeserializeOwned>(
    stream: &mut UnixStream,
    timeout: Duration,
) -> io::Result<T> {
    let deadline = Instant::now() + timeout;
    let mut header = [0; 4];
    read_bytes(stream, &mut header, deadline)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(invalid("IPC frame length outside 1..=16384"));
    }
    let mut bytes = vec![0; length];
    read_bytes(stream, &mut bytes, deadline)?;
    serde_json::from_slice(&bytes).map_err(|_| invalid("malformed IPC JSON frame"))
}

pub(super) fn write<T: Serialize>(stream: &mut UnixStream, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid("cannot encode IPC frame"))?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(invalid("IPC frame exceeds 16384 bytes"));
    }
    let deadline = Instant::now() + IO_TIMEOUT;
    write_bytes(stream, &(bytes.len() as u32).to_be_bytes(), deadline)?;
    write_bytes(stream, &bytes, deadline)
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "IPC frame deadline elapsed"))
}

fn read_bytes(stream: &mut UnixStream, mut bytes: &mut [u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        match stream.read(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated IPC frame",
                ));
            }
            Ok(count) => bytes = &mut bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn write_bytes(stream: &mut UnixStream, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_write_timeout(Some(remaining(deadline)?))?;
        match stream.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "cannot write IPC frame",
                ));
            }
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
