# `rust-mc-bot`

A high-performance stress-testing tool for custom Minecraft server implementations.

Fork of [Eoghanmc22/rust-mc-bot](https://github.com/Eoghanmc22/rust-mc-bot). All credit for the
original bot implementation goes to Eoghanmc22, who built it while working on the
[Minestom Project](https://github.com/Minestom/Minestom) — existing bot implementations would
crash before the server did, so the tool is deliberately minimal: no unnecessary features, just
raw connection throughput.

## Changes in this fork

Extended and hardened against a custom Minestom-based server, with every fix confirmed against
the target server's real packet registry (via a throwaway decompile probe against the exact
Minestom build under test) rather than assumed from the upstream protocol version.

- **Protocol version bump.** Upstream targets protocol 772 (MC 1.21.8); bumped to 776 to match
  the target server's actual handshake check.
- **Fixed a silent Configuration-state hang.** The resource-pack response only ever sent
  `ACCEPTED`, an intermediate status — the server's per-player pack future never completed, so
  the connection stalled indefinitely before Configuration could finish. Fixed by following it
  with the correct terminal status.
- **Fixed a wrong Play-state packet ID table.** Several server→client IDs (Teleport, Join Game,
  Kick, Keep Alive) were silently mapped to *other* real packets at protocol 776, so the bot never
  recognized a teleport confirmation and never answered keep-alives — causing a silent ~15s
  timeout kick with no visible connection error. Every corrected ID was verified against the
  target server's live packet registry.
- **Fixed drifted client→server Play-state packet IDs** (chat, animation, player action, held
  item, keep alive, player position) that had also shifted between protocol versions, and were
  previously being silently no-op'd by the server rather than rejected — meaning prior test runs
  validated connection capacity but not much of what happened after joining.
- **Added a missing `Player Loaded` packet** on entering Play state — without it, server-side
  per-player state that's only initialized in response to that packet was never created for any
  bot, breaking anything gated on it.
- **Added real interaction load**, not just idle connections: periodic block-breaking, and
  block-placement wired to a subset of bots to exercise attack/contested-territory server logic
  under concurrent load.
- **Fixed spawn clustering.** Bots previously piled up at a single shared coordinate; entity
  tracking cost scales with local density, not raw count, so a tight pile is a much heavier (and
  much less representative) load than the same bot count spread across the map. Bots now get a
  randomized spawn offset.
- **Added a CI-triggered load-test workflow** (`.github/workflows/loadtest.yml`) — builds and runs
  the bot swarm against a configurable target/count/duration on a free GitHub-hosted runner, no
  dedicated always-on machine required.

## Usage

1. Clone the code
    - To stress-test a server on an older protocol version, check out the corresponding tag:
      ```bash
      git checkout tags/1.18.1
      ```
2. Compile the code
    - Requires Rust — see [rustup.rs](https://rustup.rs).
      ```bash
      cargo build --release
      ```
    - Executable is built to `target/release/rust-mc-bot` (Linux/macOS) or
      `target/release/rust-mc-bot.exe` (Windows).
3. Start the bots
    ```bash
    ./rust-mc-bot <ip:port or path> <count> [threads]
    ./rust-mc-bot 127.0.0.1:25565 1000
    ```
    or
    ```bash
    cargo run --release -- <ip:port or path> <count> [threads]
    cargo run --release -- 127.0.0.1:25565 1000
    ```

### Via GitHub Actions

The `Load test` workflow (`workflow_dispatch`) builds and runs the bot swarm against a
configurable `target`, `count`, and `duration_seconds` directly from the Actions tab — useful for
running a load test without a dedicated machine.

## Known Issues

Using `localhost` as the IP on machines with IPv6 may cause the bots to fail to connect. Use
`127.0.0.1` instead.

The bots do not support online mode, to prevent abuse and improve performance.

## Disclaimer

This should **only** be used to test servers you own or are authorized to test. Running this
against a server you don't control or have permission to test can be considered illegal, as it
simulates a layer-7 denial-of-service attack.

## License

GPL-3.0, inherited from upstream. See [`LICENSE`](LICENSE).
