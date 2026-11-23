# Profile local XR controls in Sakura Twilight

Use one live headset and a synthetic peer. The peer uses the normal
multiplayer handshake and typed UDP channels and runs without a second scene,
renderer, or OpenXR session. Loopback works on the headset PC. Running the
synthetic peer on another LAN machine also removes its small CPU load from the PC.

Build both binaries before sampling:

```sh
cargo build --release --example sakura-twilight-test-host --example profile-sakura-xr
```

Start the host in one terminal. Choose one row per capture:

| Purpose | Host command |
| --- | --- |
| Connected with ordinary outgoing client pose traffic | `target/release/examples/sakura-twilight-test-host session` |
| Incoming 20 Hz snapshots of a stationary airborne avatar | `target/release/examples/sakura-twilight-test-host still` |
| Same channels and cadence with moving root, head and hands | `target/release/examples/sakura-twilight-test-host motion` |
| Same motion with 80 ms added before host pose delivery | `target/release/examples/sakura-twilight-test-host motion 80` |

On the headset PC run the client. Use a different report directory for each
row, such as `/tmp/sakura-session`, `/tmp/sakura-still`,
`/tmp/sakura-motion`, and `/tmp/sakura-motion-80`.

```sh
MITTENS_MULTIPLAYER_ROLE=client \
MITTENS_MULTIPLAYER_NAME=rei \
MITTENS_MULTIPLAYER_ADDR=127.0.0.1:7010 \
target/release/examples/profile-sakura-xr twilight /tmp/sakura-motion-80
```

The capture waits for a peer and a rendered scene, warms up for 10 seconds,
then samples for 15 seconds and exits. If the peer disconnects, the profiler
waits for it to reconnect and starts a fresh warm-up. For longer runs, append
`--warmup 30 --sample 60 --startup-timeout 300` to the headset command. The
startup allowance is in addition to the timed warm-up and sample.

Compare the same headset settings and
route each time. During each capture, turn your head, move your hands, and
walk the same short path with the stick. Compare submitted projection FPS
and p95 frame time, then look at native networking, MMS callback service,
XR locomotion and GPU completion. A near-constant delay on the remote motion
with stable local frame timing is expected in the 80 ms run.

For a LAN run, add a bind IPv4 address on the host, such as
`sakura-twilight-test-host motion 80 0.0.0.0`, and use the host's LAN IP in
`MITTENS_MULTIPLAYER_ADDR`. Control uses UDP 7010. Typed traffic uses UDP
7011 on the host and UDP 7012 on the client.

The 80 ms queue affects avatar samples sent from the synthetic peer to the headset.
Session control, headset input and headset-to-peer traffic keep their normal
timing. The harness sends no prop or vehicle updates. You can run the same
capture against a real Sakura Twilight host on LAN to include that traffic.
The report counts app-submitted projections and the runtime's requested
display interval. Compositor reprojection needs the XR runtime's own timing.

## Swap roles for the comparison

Stop the synthetic host to free UDP 7010. Start the headset as host:

```sh
MITTENS_MULTIPLAYER_ROLE=host \
MITTENS_MULTIPLAYER_NAME=rei \
MITTENS_MULTIPLAYER_ADDR=127.0.0.1:7010 \
target/release/examples/profile-sakura-xr twilight /tmp/sakura-host-motion-80 --wait-for-peer
```

In the other terminal start the synthetic client:

```sh
target/release/examples/sakura-twilight-test-host --client motion 80
```

The client uses the same movement and adds the same 80 ms delay to outgoing
pose samples. Use the warm-up to match the viewing position and direction
from the client run, since Sakura spawns the roles on opposite sides of the
roof. Keep the same MSAA, XR render resolution, and route. Compare the two
reports' submitted projection FPS, p95 headset frame time, runtime-predicted
display interval, and native network, update, and rendering times.

Across a LAN, bind the headset host to `0.0.0.0:7010` and append its LAN IPv4
address to the synthetic client command.

For the lower-density scene, pass `twilight-simpler` to the headset profiler
and choose a separate report directory. The synthetic peer command is the
same. With two real players, load `examples/sakura-twilight-simpler.mms` on
both machines.
