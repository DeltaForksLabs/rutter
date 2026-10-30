use std::{
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};

use rutter::activity::{
    ActivityAction, ActivityContent, ActivityError, Media, MediaState, Transfer, TransferDirection,
    TransferState, ipc::ActivityClient,
};

const TOTAL: u64 = 10 * 1024 * 1024;
const TRANSFER_TIME: Duration = Duration::from_secs(30);
const TRACK_TIME: Duration = Duration::from_secs(180);
const TICK: Duration = Duration::from_millis(100);
const USAGE: &str = "usage: dynamic_island_client <socket> download|upload|media [--unknown-total]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Download,
    Upload,
    Media,
}

struct Options {
    socket: PathBuf,
    kind: Kind,
    unknown_total: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, &'static str> {
        let mut args = args.into_iter();
        let socket = PathBuf::from(args.next().ok_or(USAGE)?);
        let kind = match args.next().as_deref().and_then(|value| value.to_str()) {
            Some("download") => Kind::Download,
            Some("upload") => Kind::Upload,
            Some("media") => Kind::Media,
            _ => return Err(USAGE),
        };
        let unknown_total = match args.next() {
            None => false,
            Some(flag) if flag == "--unknown-total" && kind != Kind::Media => true,
            _ => return Err(USAGE),
        };
        if args.next().is_some() {
            return Err(USAGE);
        }
        Ok(Self {
            socket,
            kind,
            unknown_total,
        })
    }
}

struct Simulation {
    kind: Kind,
    unknown_total: bool,
    elapsed: Duration,
    paused: bool,
    cancelled: bool,
    track: usize,
    completed_for: Duration,
}

impl Simulation {
    fn new(options: &Options) -> Self {
        Self {
            kind: options.kind,
            unknown_total: options.unknown_total,
            elapsed: Duration::ZERO,
            paused: false,
            cancelled: false,
            track: 0,
            completed_for: Duration::ZERO,
        }
    }

    // The clock is injected as elapsed time; tests never sleep or access a device.
    fn advance(&mut self, elapsed: Duration) {
        if self.cancelled || self.paused {
            return;
        }
        let limit = if self.kind == Kind::Media {
            TRACK_TIME
        } else {
            TRANSFER_TIME
        };
        if self.elapsed == limit && self.kind != Kind::Media {
            self.completed_for = self.completed_for.saturating_add(elapsed);
        }
        self.elapsed = self.elapsed.saturating_add(elapsed).min(limit);
    }

    fn action(&mut self, action: ActivityAction) {
        // A request accepted before the final progress tick may arrive after
        // completion. Do not let a stale Pause prevent automatic retirement.
        if self.kind != Kind::Media
            && self.elapsed == TRANSFER_TIME
            && action != ActivityAction::Cancel
        {
            return;
        }
        match action {
            ActivityAction::Pause => self.paused = true,
            ActivityAction::Resume => {
                self.paused = false;
                if self.kind == Kind::Media && self.elapsed == TRACK_TIME {
                    self.elapsed = Duration::ZERO;
                }
            }
            ActivityAction::Cancel if self.kind != Kind::Media => self.cancelled = true,
            ActivityAction::Previous | ActivityAction::Next if self.kind == Kind::Media => {
                self.track = if action == ActivityAction::Next {
                    (self.track + 1) % 3
                } else {
                    (self.track + 2) % 3
                };
                self.elapsed = Duration::ZERO;
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> Result<ActivityContent, ActivityError> {
        if self.kind == Kind::Media {
            let title = [
                "Simulation · Morning orbit",
                "Simulation · Quiet coast",
                "Simulation · Night train",
            ][self.track];
            let state = if self.paused {
                MediaState::Paused
            } else if self.elapsed == TRACK_TIME {
                MediaState::Stopped
            } else {
                MediaState::Playing
            };
            return Ok(Media::new(
                title,
                "Demo artist · no audio playback",
                self.elapsed,
                Some(TRACK_TIME),
                state,
            )?
            .into());
        }
        let completed =
            (self.elapsed.as_millis() * u128::from(TOTAL) / TRANSFER_TIME.as_millis()) as u64;
        let state = if completed == TOTAL {
            TransferState::Completed
        } else if self.paused {
            TransferState::Paused
        } else {
            TransferState::Running
        };
        let direction = if self.kind == Kind::Download {
            TransferDirection::Download
        } else {
            TransferDirection::Upload
        };
        Ok(Transfer::new(
            "Simulated 10 MiB transfer",
            direction,
            completed,
            if self.unknown_total {
                None
            } else {
                Some(TOTAL)
            },
            state,
        )?
        .with_supporting("Simulation only · units are bytes · no network I/O")?
        .into())
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse(std::env::args_os().skip(1))?;
    let mut simulation = Simulation::new(&options);
    let mut client = ActivityClient::connect(&options.socket)?;
    let mut published = simulation.snapshot()?;
    let id = client.start(published.clone())?;
    println!(
        "Published {:?} simulation. Use host controls; Ctrl+C disconnects and removes its activities.",
        options.kind
    );
    let mut previous = Instant::now();
    loop {
        std::thread::sleep(TICK);
        let now = Instant::now();
        // Advance before applying new controls so paused time is not counted on
        // resume, and a track skip starts at zero rather than at the old tick.
        simulation.advance(now.duration_since(previous));
        previous = now;
        // Drain at most one queue's worth before publishing a tick, so ongoing
        // control traffic cannot indefinitely starve progress updates.
        for _ in 0..rutter::activity::ipc::MAX_PENDING_ACTIONS {
            let Some(event) = client.poll_action()? else {
                break;
            };
            if event.id == id {
                println!("Producer received {:?}", event.action);
                simulation.action(event.action);
            }
        }
        if simulation.cancelled || simulation.completed_for >= Duration::from_secs(3) {
            client.remove(id)?;
            println!("Simulation removed.");
            return Ok(());
        }
        let snapshot = simulation.snapshot()?;
        if snapshot != published {
            client.replace(id, snapshot.clone())?;
            published = snapshot;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(kind: &str) -> Options {
        Options::parse(["/private/activity.sock", kind].map(OsString::from)).unwrap()
    }

    #[test]
    fn parsing_requires_explicit_socket_and_approved_kind() {
        for kind in ["download", "upload", "media"] {
            assert_eq!(
                options(kind).socket,
                PathBuf::from("/private/activity.sock")
            );
        }
        for args in [
            vec![],
            vec!["/socket"],
            vec!["/socket", "command"],
            vec!["/socket", "media", "--unknown-total"],
            vec!["/socket", "upload", "extra"],
        ] {
            assert!(Options::parse(args.into_iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn transfer_pause_resume_completion_cancel_and_unknown_total() {
        let mut simulation = Simulation::new(&options("upload"));
        simulation.advance(Duration::from_secs(15));
        let ActivityContent::Transfer(t) = simulation.snapshot().unwrap() else {
            panic!()
        };
        assert_eq!(t.completed(), TOTAL / 2);
        assert_eq!(t.direction(), TransferDirection::Upload);
        simulation.action(ActivityAction::Pause);
        simulation.advance(Duration::from_secs(100));
        assert_eq!(simulation.elapsed, Duration::from_secs(15));
        simulation.action(ActivityAction::Resume);
        simulation.advance(Duration::from_secs(20));
        let ActivityContent::Transfer(t) = simulation.snapshot().unwrap() else {
            panic!()
        };
        assert_eq!(t.state(), TransferState::Completed);
        simulation.unknown_total = true;
        let ActivityContent::Transfer(t) = simulation.snapshot().unwrap() else {
            panic!()
        };
        assert_eq!(t.total(), None);
        simulation.action(ActivityAction::Cancel);
        assert!(simulation.cancelled);
    }

    #[test]
    fn media_skip_and_pause_preserve_producer_owned_position() {
        let mut simulation = Simulation::new(&options("media"));
        simulation.advance(Duration::from_secs(61));
        simulation.action(ActivityAction::Pause);
        simulation.advance(Duration::from_secs(20));
        let ActivityContent::Media(m) = simulation.snapshot().unwrap() else {
            panic!()
        };
        assert_eq!(m.position(), Duration::from_secs(61));
        assert_eq!(m.state(), MediaState::Paused);
        simulation.action(ActivityAction::Next);
        assert_eq!(simulation.track, 1);
        assert_eq!(simulation.elapsed, Duration::ZERO);
        simulation.action(ActivityAction::Previous);
        assert_eq!(simulation.track, 0);
        simulation.action(ActivityAction::Resume);
        simulation.advance(Duration::from_secs(2));
        assert_eq!(simulation.elapsed, Duration::from_secs(2));
        simulation.advance(TRACK_TIME);
        simulation.action(ActivityAction::Resume);
        assert_eq!(simulation.elapsed, Duration::ZERO);
    }

    #[test]
    fn pause_accepted_before_completion_does_not_prevent_retirement() {
        let mut simulation = Simulation::new(&options("download"));
        simulation.advance(TRANSFER_TIME);
        simulation.action(ActivityAction::Pause);
        simulation.advance(Duration::from_secs(3));
        assert_eq!(simulation.completed_for, Duration::from_secs(3));
        let ActivityContent::Transfer(snapshot) = simulation.snapshot().unwrap() else {
            panic!()
        };
        assert_eq!(snapshot.state(), TransferState::Completed);
        simulation.action(ActivityAction::Cancel);
        assert!(simulation.cancelled);
    }
}
