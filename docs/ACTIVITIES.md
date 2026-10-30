# Activities and optional Unix IPC

`rutter::activity` is an in-process, renderer-independent model for owned transfer
and media snapshots. It has no callbacks, I/O or runtime dependency. The optional
`activity-ipc` Cargo feature adds a synchronous Unix-domain socket protocol and
host/client examples. Neither layer performs network transfers or audio playback.

## In-process ownership, snapshots and routes

`ActivityRegistry::register_owner` allocates an opaque `OwnerId`; `start` allocates
an opaque `ActivityId`. IDs are monotonically allocated per kind across the host
process, including independent registries and clones, and are never reused after
removal. An ID must be present in the registry where it is used. IDs are
identifiers, not authentication credentials.

`replace(owner, id, content)` replaces the entire snapshot, including its kind,
while preserving owner, activity ID and selection. There are no partial updates
or implicit progress/playback advancement. `remove` checks ownership;
`remove_owner` removes the owner and all its activities. Failed operations leave
the registry intact; capacity errors do not evict another activity.

The first activity selects itself. Later starts and replacements preserve the
selection; `select(id)` changes it explicitly. Removing the selected activity
selects the oldest remaining activity (lowest ID), or none. `iter()` follows
that deterministic ID order. Registry clones are independent snapshots, not
shared mutable state.

```rust
use rutter::activity::{ActivityAction, ActivityRegistry, Transfer,
    TransferDirection, TransferState};

let mut registry = ActivityRegistry::new();
let owner = registry.register_owner()?;
let id = registry.start(owner, Transfer::new(
    "Archive", TransferDirection::Download, 25, Some(100), TransferState::Running,
)?.with_supporting("Producer-defined units")?)?;
registry.select(id)?;

// Validate against the latest snapshot; this does not pause it or deliver I/O.
let event = registry.route_action(id, ActivityAction::Pause)?;
assert_eq!(event.owner(), owner);
assert_eq!(event.id(), id);
assert_eq!(event.action(), ActivityAction::Pause);

// After the owning producer handles the request, publish its new snapshot.
registry.replace(owner, id, Transfer::new(
    "Archive", TransferDirection::Download, 25, Some(100), TransferState::Paused,
)?)?;
registry.remove(owner, id)?;
registry.remove_owner(owner)?;
# Ok::<(), rutter::activity::ActivityError>(())
```

`route_action` returns an `ActivityActionEvent` with `owner()`, `id()` and
`action()`. Delivery and subsequent producer updates are the caller's
responsibility. Revalidate messages from an older rendered view against the
latest registry before dispatching them.

| Snapshot state | Supported actions |
| --- | --- |
| Transfer `Running` | `Pause`, `Cancel` |
| Transfer `Paused` | `Resume`, `Cancel` |
| Transfer `Completed` | none |
| Media `Playing` | `Pause`, `Previous`, `Next` |
| Media `Paused` or `Stopped` | `Resume`, `Previous`, `Next` |

`Resume` can request starting stopped media. `Cancel` is transfer-only.

### Validation and fixed model limits

- At most 16 registered owners, 64 live activities overall and 8 per owner.
- Titles: at most 256 UTF-8 **bytes**. Transfer supporting text and media artist:
  at most 2048 bytes each. Empty strings are allowed; overlong text is rejected,
  never truncated. Text is plain display content, not markup, paths, artwork,
  commands or executable instructions.
- Transfer units are producer-defined `u64` counts. A known total may be zero;
  completed units cannot exceed it. A `Completed` snapshot with a known total
  must exactly reach that total. Unknown totals permit any completed count.
- `Transfer::percentage()` is `None` for unknown totals. Zero/zero is 100% only
  when completed, otherwise 0%; large counts do not use overflowing integer products.
- Media uses `std::time::Duration` for position and optional duration. Position
  may equal, but cannot exceed, a known duration. Paused/stopped positions are
  retained. Live/unknown duration uses `None`.

## Dynamic Island integration

The registry does not choose a widget or native surface. `activity::view::summary`
formats English plain text; `details` builds wrapped title/supporting text,
progress and supported semantic action buttons. Known transfer totals use
determinate progress. Unknown running totals use indeterminate progress;
paused/completed unknown totals do not animate. Media position is displayed,
never advanced by the view. The parent owns padding and scrolling; details own
12px sibling gaps and wrap action rows with 8px horizontal and vertical gaps.
Controls use structural/automatic identity; callbacks carry the activity ID.

```rust
use rutter::{DynamicIsland, Theme};
use rutter::activity::{ActivityAction, ActivityId, ActivityRegistry,
    Transfer, TransferDirection, TransferState, view};

#[derive(Clone, Debug)]
enum Message { ToggleDetails, Action(ActivityId, ActivityAction) }

let mut registry = ActivityRegistry::new();
let owner = registry.register_owner()?;
let id = registry.start(owner, Transfer::new(
    "Upload", TransferDirection::Upload, 40, Some(100), TransferState::Running,
)?)?;
let activity = registry.get(id).expect("just started");
let summary = view::summary(activity);
let theme = Theme::light();

let passive = DynamicIsland::passive();
let passive_widget = passive.passive_view::<Message>(&summary, &theme)?;

let mut interactive = DynamicIsland::interactive();
interactive.set_expanded(true)?;
let interactive_summary = format!("{summary} · Collapse details");
let details = view::details(activity, Message::Action, &theme);
let interactive_widget = interactive.interactive_view(
    &interactive_summary, Message::ToggleDetails, details, &theme,
)?;
// The reducer handles ToggleDetails with interactive.toggle(), and handles
// Action(id, action) with registry.route_action(id, action) before delivery.
# Ok::<(), Box<dyn std::error::Error>>(())
```

Passive islands get text only, without controls. Collapsed interactive islands
exclude details from layout, hit testing and accessibility. Retain summary text
for the island view's lifetime. The supplied host uses this actual composition,
not a text-only stand-in for the activity details.

## Unix host and client examples

The feature is off by default. `activity::ipc` is compiled only under
`cfg(all(unix, feature = "activity-ipc"))`. In-process activity APIs remain
available on other platforms. The example targets require the feature; on
non-Unix their entry points return a Unix-required error rather than providing
a fallback transport. The optional Unix dependencies are `serde`, `serde_json`
and `libc`, as declared in `Cargo.toml`.

The synchronous client takes an explicit socket path, not the directory. Run its
blocking calls outside the GUI thread. This function illustrates a producer
publishing media and handling a Pause request with a full replacement:

```no_run
#[cfg(all(unix, feature = "activity-ipc"))]
fn publish(socket: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;
    use rutter::activity::{ActivityAction, Media, MediaState, ipc::ActivityClient};

    let mut client = ActivityClient::connect(socket)?;
    let id = client.start(Media::new(
        "Track", "Artist", Duration::ZERO, None, MediaState::Playing,
    )?)?;
    if let Some(event) = client.wait_action(Duration::from_secs(1))? {
        if event.id == id && event.action == ActivityAction::Pause {
            // Publish only after the producer has actually paused its own work.
            client.replace(id, Media::new(
                "Track", "Artist", Duration::ZERO, None, MediaState::Paused,
            )?)?;
        }
    }
    client.remove(id)?;
    Ok(())
}
```

In the host terminal, create a short, private directory and run the GUI:

```sh
activity_dir=$(mktemp -d /tmp/rutter-activity.XXXXXX)
chmod 700 "$activity_dir"
cargo run --locked --offline --features activity-ipc --example dynamic_island_host -- "$activity_dir"
# After closing the host and stopping clients, remove the empty directory:
rmdir "$activity_dir"
```

The host prints its absolute socket path. Wait for its window to open before
connecting: socket creation precedes GUI initialization, and requests made while
the GUI is still starting may hit the two-second acceptance deadline. Retry with
a fresh client after startup. Copy the path into a second terminal;
replace the placeholder suffix below with the one actually created. Run a client
mode at a time, or use additional terminals for multiple live producers:

```sh
cargo run --locked --offline --features activity-ipc --example dynamic_island_client -- /tmp/rutter-activity.XXXXXX/activity.sock download
cargo run --locked --offline --features activity-ipc --example dynamic_island_client -- /tmp/rutter-activity.XXXXXX/activity.sock upload --unknown-total
cargo run --locked --offline --features activity-ipc --example dynamic_island_client -- /tmp/rutter-activity.XXXXXX/activity.sock media
```

`--unknown-total` is accepted only for `download`/`upload`. The simulated transfer
uses byte units and completes over 30 seconds of running time, then retires after
three more seconds; Cancel removes it. Media simulates three 180-second tracks;
Previous/Next change tracks. Pause/Resume affect producer-owned time. Ctrl+C
disconnects the client, causing host-side owner/activity cleanup when ingress is
applied. There is no actual network transfer, audio device, shell command or artwork.

The host defaults to an **ordinary 800×700 window**, with a scrolling document,
activity selectors, theme switching, controlled expansion and visible failure
feedback. Its optional native mode is described below; `--example dynamic_island`
remains the separate animation/hover demonstration.
Run resource-intensive Cargo operations sequentially; the commands above are
usage instructions, not validation results.

### Optional native overlay

Append the flag **after the existing private directory**:

```sh
cargo run --locked --offline --features activity-ipc --example dynamic_island_host -- "$activity_dir" --overlay
```

The socket, client commands, activity selection and producer ownership are
unchanged. Do not start both host modes in the same directory at once. Omitting
the flag preserves the ordinary-window default. Unknown/duplicate flags and extra
arguments fail before IPC bind; paths beginning with `-` need an explicit `./`.

Native mode requires Wayland, layer-shell **v4 or newer** (for OnDemand keyboard
policy) and an alpha-capable rendering backend. It does not silently fall back on
X11 or a compositor lacking the required protocol. Runner startup errors still
run IPC shutdown/endpoint cleanup. Output selection is compositor-chosen; this
mode has no monitor-selection flag or cross-platform native fallback.

The host is a transparent, undecorated, nonresizable **420×300 logical-pixel**
Overlay-layer surface, anchored at the top center without reserving work area.
Native input starts Empty. Actual configured metrics determine a capsule within
an 8px host inset: compact at most 240×76, expanded at most 404×284. These are
viewport/control dimensions, not text-height guarantees or spacing tokens. The
same logical rectangle drives layout and rounded native input, with scaling
applied by the native boundary only. Margins and rounded corners pass clicks to
the underlying application. Invalid or unusably small metrics leave input Empty.

The capsule starts collapsed. Its header shows the selected activity or an
actionable empty-host summary. Clicking the header toggles immediate expansion;
expanded content includes activity selection, theme switching, error feedback,
producer controls and **Close host**. The entire natural-height document scrolls,
including the header, so long summaries cannot permanently hide controls. Scroll
to the top to reach the header, or use Escape when the overlay has keyboard focus.
Disconnecting the last producer restores the empty summary rather than hiding
the surface, keeping theme and close controls available.

Keyboard policy starts OnDemand. A **user-driven expansion** requests Exclusive
once, with a 750ms focus-grant deadline unless already focused. A focus grant
cancels the deadline; expiry or focus loss restores OnDemand while leaving details
open for click focus. There is no repeated regrab loop. Escape, collapse and Close
restore OnDemand. Incoming producer updates, selection fallback, actions and theme
updates never request keyboard focus or expand the capsule. There is no hover
expansion or geometry/text animation in this mode.

The example keeps native policy in `dynamic_island_host/app/overlay/` and its
parent module; registry, ingress and I/O supervision remain shared with the
ordinary host. Input publication deduplicates unchanged regions and republishes
after metric changes/recreation. Close cancels deferred work and clears input
before closing; the existing supervisor cleanup handles both modes.

An owned-compositor smoke driver exercises the actual host, raw Unix producers,
rounded click-through, compositor pixels, focus/Escape, scrolling, Pause/Resume,
disconnect cleanup, Close and endpoint reuse. Build first, then run sequentially:

```sh
CARGO_BUILD_JOBS=2 cargo build --locked --offline --features activity-ipc --example dynamic_island_host --example dynamic_island_probe
python3 -B tests/native/activity_overlay_smoke.py --labwc /absolute/path/to/labwc --xvfb /absolute/path/to/Xvfb --keep
```

`--fraym /absolute/path/to/fraym` is available instead of `--labwc`. The driver
creates its own display/compositor, private configuration and runtime directory;
it never uses the inherited desktop. It requires real Escape acceptance and also
checks explicit X11 rejection/endpoint cleanup on its owned X display. Native
coverage is limited to the compositor actually run, 1100×800 at 1x, light theme and
short transfer text; it is not manual visual, screen-reader or cross-platform
acceptance. Geometry/software-raster tests cover the broader size/theme/scale
matrix separately. Actual execution outcomes are recorded in the development log.

## Version-1 wire protocol

Each frame is a four-byte unsigned **big-endian** payload length followed by
UTF-8 JSON. The length excludes the header and must be 1..=16384 bytes. Header
and payload share one frame deadline. Operations and content use Serde's
externally tagged representation; names and enum values are case-sensitive.

| Client request JSON | Successful reply JSON |
| --- | --- |
| `{"Hello":{"version":1}}` (first frame only) | `"Ready"` |
| `{"Start":{"Transfer":{"title":"Archive","supporting":"","direction":"Download","completed":0,"total":100,"state":"Running"}}}` | `{"Started":123}` (example handle) |
| `{"Replace":{"id":123,"content":{"Media":{"title":"Track","artist":"Artist","position":{"secs":0,"nanos":0},"duration":null,"state":"Playing"}}}}` | `"Accepted"` |
| `{"Remove":{"id":123}}` | `"Accepted"` |
| `"Poll"` | `{"Action":null}` or `{"Action":{"id":123,"action":"Pause"}}` |

Failures use `{"Error":"message"}`. Unsupported/missing initial handshake is
rejected; repeated Hello is an error. Request/content objects reject unknown
fields. Content has the same validated fields and state/action names as the
in-process model. Media durations use `{"secs":integer,"nanos":integer}` rather
than floating-point seconds; optional totals/durations can be `null`.

Hello registers one owner for that connection. Clients never send an owner ID.
Numeric activity handles are looked up only in their originating connection;
a handle from another connection is rejected. Replacement is a full snapshot
and may switch Transfer to Media or vice versa. Selection remains host-owned.
There is no client-supplied action callback, arbitrary widget tree, URL fetching,
path execution or remote command operation.

### Deadlines, backpressure and action delivery

- At most 8 connections (including disconnected sessions awaiting cleanup), one
  ordinary GUI-acceptance request per connection, and 32 pending actions per
  connection. The model limits above also apply.
- Connect, frame reads/writes and GUI acceptance each have a two-second deadline.
  Client reply reads allow three seconds for GUI scheduling. An idle client must
  poll within the two-second input interval to remain connected.
- Client calls are synchronous and serialized. `poll_action` returns at most one
  queued action without waiting for a future one; actions arrive only in poll
  replies. `wait_action(timeout)` repeatedly polls; it checks the caller's
  timeout between exchanges, so an in-flight exchange may take up to five seconds.
- Transport/protocol errors poison `ActivityClient`; reconnect explicitly.
  A remote validation rejection does not itself poison the connection.
- `ActivityServer::dispatch(&registry, event)` revalidates against the live
  registry and connection ownership. Full or contended queues return
  `WouldBlock`; success means **queued**, not executed or acknowledged.
- Accepted actions remain **FIFO across replacements**, including kind/state
  changes that no longer support the old request. Producers must decide how to
  handle these past requests. Removal or disconnect discards pending actions for
  removed activities. There is no delivery guarantee across transport failure,
  acknowledgement, persistent queue or replay protocol.

## Host lifecycle and trust boundary

`ActivityServer::bind(private_directory, deliver)` requires an existing real
directory, not a final symlink, owned by the effective UID with exactly mode
0700. Ancestors are trusted host input. It creates `activity.sock` with mode
0600, refuses to replace an existing endpoint, and uses no TCP transport. Keep
the path short enough for the platform's Unix socket path limit.

This is **same-UID trust**, not general authentication or isolation from other
processes running as that UID. Filesystem privacy is the access boundary; the
protocol's owner mapping prevents cross-connection activity modification, not
same-UID impersonation of a host. No separate peer-credential authentication is
provided.

Keep the I/O supervisor separate from cloneable GUI states. The ingress callback
must promptly enqueue `HostRequest` into a **bounded FIFO** and never apply it
inline on a worker. Return `DeliveryError::Full` for retry or `Closed` when the
GUI loop is gone. In the GUI reducer, apply requests in FIFO order to the **same
authoritative registry**. Cloned requests share an acceptance ticket and apply
at most once; applying to a per-window snapshot does not synchronize the model.
Timed-out/disconnected requests cannot later create owners or activities.

Disconnect cleanup is retained and retried while the server runs, including
when ingress is full. Shutdown requires explicit host participation:

1. Call `ActivityServer::shutdown()` to stop acceptance, cancel sessions, join
   I/O threads and attempt socket removal.
2. Apply **every** `ShutdownReport::cleanup` request to the authoritative registry,
   even if GUI ingress is closed or endpoint removal failed.
3. Inspect/report `ShutdownReport::endpoint_error`. Queued requests may safely be
   drained afterward; cleanup is idempotent.

Shutdown unlinks only the socket inode the server owns, refusing to remove a
replacement. The private directory remains the caller's responsibility. With a
nonblocking callback, shutdown is bounded by one read and write deadline plus
scheduling overhead; blocking callbacks invalidate that bound. Dropping the
server stops I/O but cannot mutate a GUI-owned registry or apply its cleanup
report. Do explicit cleanup before discarding or continuing to use that registry.

If reading the endpoint's metadata fails immediately after a successful bind,
startup fails without an established inode identity. The endpoint may remain;
inspect the private directory before manually removing that failed-startup socket.
The server deliberately does not unlink an endpoint whose identity it cannot verify.

The supplied host retains its supervisor outside UI state clones, shares only
references without clone/drop lifecycle side effects, and retains the latest
authoritative registry snapshot for cleanup if startup/rendering failure skips
the normal surface-close callback. See
`examples/apps/dynamic_island_host/app.rs` for the bounded message-ingress and
shutdown integration, and `src/activity/ipc.rs` for the public host API example.

## Validation scope

The model, transport and examples include deterministic regression tests for
validation, ownership, selection, routing, framing, lifecycle and presentation.
Their presence is not a claim that this documentation edit ran them. Builds,
doctests and other validation outcomes belong in the final validation report.
No cross-platform GUI, manual visual or live assistive-technology acceptance is
claimed. The owned native driver records only the compositor and checks actually
executed; its availability is not evidence that every compositor was tested.
Unix-only IPC availability does not establish runtime acceptance on every Unix
platform.
