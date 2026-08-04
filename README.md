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
  block-placement wired to a subset of bots (`ATTACK_FRACTION`) targeting real border-chunk
  coordinates, to exercise the target server's contested-territory (war-flag) attack path under
  concurrent load. This required chasing down four separate, entirely independent bugs before a
  single attack packet actually produced a real server-side attack — each one silently swallowed
  the attempt with no visible error, so the only way to tell them apart was fixing one at a time
  and re-verifying against the server's real response packet (see "Confirming the fix actually
  worked" below), not by reading logs:
  1. **Wrong target coordinates.** The original test territories didn't border each other and, for
     a while, didn't exist in the target server's real world at all. Fixed by switching to two
     real, adjacent, currently-unclaimed production territories and computing their 7 real
     shared-border chunk pairs directly from the live world data (not assumed) — see
     `TOWN_A_ATTACK_TARGETS` / `TOWN_B_ATTACK_TARGETS` in `src/main.rs`.
  2. **Repeated targets under concurrency.** Attacking bots picked a target via
     `bot.id/2` stepped by `ATTACK_FRACTION`, which could hand the same chunk to multiple
     concurrent attackers well before every entry in the target table had been used (the server
     only allows one active attack per chunk, so this silently capped real concurrent load far
     below what the bot count implied). Fixed with `NEXT_TOWN_A_TARGET`/`NEXT_TOWN_B_TARGET`,
     monotonic atomic counters that hand out a genuinely distinct target per attacker, wrapping
     only past 7 concurrent attackers per side.
  3. **`bot.id` was not actually unique.** `start_bots` spawns one thread per CPU, and `id:` was set
     from the *per-thread-local* loop variable rather than a globally unique number — with more
     CPUs than bots-per-thread, almost every bot's local index came out `0`, silently breaking the
     `bot.id % N` parity/fraction checks this entire feature depends on. Fixed by deriving `id`
     from `name_offset + bot` instead.
  4. **The attacking bot's own hitbox blocked its own placement.** Bots used to move to stand
     *exactly* at the block they were about to place (`(tx+0.5, ground_y+1, tz+0.5)` — the same
     cell as the new flag block). Confirmed by decompiling the target server's exact Minestom
     build: `BlockPlacementListener` calls `CollisionUtils.canPlaceBlockAt` *before* the server
     ever constructs or dispatches `PlayerBlockPlaceEvent`, and when the returned colliding entity
     is the placing player itself, the server just acknowledges the packet and returns — no event,
     no error, no chat message, nothing observable at all. Fixed by standing 1 block off the
     target column instead of on top of it (see the comment at the placement call site in
     `main.rs`).

  Bugs 1-3 also required matching fixes server-side (real territory/nation/war-state setup) —
  documented in that project, not here.
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

## Confirming the war-flag attack actually worked

Every bug above produced the exact same symptom: the bot connects fine, sends a structurally valid
block-placement packet, and then — nothing. No kick, no console error, no visible difference at
all between "the attack succeeded" and "the attack silently failed a precondition." Two channels
that seem like they should tell you which one happened, don't:

- **The target server's console log.** Its `[War]`/error broadcasts go through the server's own
  chat/messaging API, which routes through the network layer to connected clients — never through
  `println` to the console. A clean console proves nothing either way.
- **Territory/town save files after the fact.** A border-chunk capture (as opposed to a
  core-chunk capture) only ever sets in-memory, runtime-only state — nothing about it is written
  to the on-disk world/town data at all. An unclaimed-looking chunk in a save file is consistent
  with both "never attacked" and "attacked and captured."

The only channel that can actually distinguish success from failure is the real chat packet the
server sends back to the connecting bot: a success message (`"... is attacking ..."`) or a specific
error reason, both delivered as a `SystemChatPacket` (packet ID confirmed at runtime against the
target server's exact Minestom build, not assumed — see the target project's own notes on how).
`states/play.rs` includes a handler for it that does a crude printable-ASCII scan of the packet's
payload bytes and prints whatever text it finds — not a real NBT/Component decoder, just enough to
read the message text back, which is all this needed. Reading that text is what actually caught bug
4 above, after bugs 1-3 were already fixed and the attack was *still* silently failing — every other
signal available said everything was fine.

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
