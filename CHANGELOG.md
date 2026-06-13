# Changelog

All notable changes to Aurelia will be documented in this file.

## Unreleased

## 0.2.1 - Save Compatibility Hardening (2026-06-12)

### Added

- Vanilla Beta 1.7.3 world save foundation with save/load round-trip support.

### Changed

- Comprehensive tests for vanilla save compatibility paths.

### Fixed

- Clippy hygiene cleanup across the workspace.

## 0.2.2 - Zero-Argument Launch and server.properties (2026-06-13)

### Added

- `server.properties` support: Aurelia reads standard vanilla-style configuration from `server.properties` in the working directory, generating a commented default file on first run. Supported keys: `server-port`, `server-ip`, `level-name`, `motd`, `max-players`, `online-mode`, `view-distance`. Keys `spawn-protection`, `white-list`, `allow-nether`, `difficulty`, and `gamemode` are parsed and warned about, but not yet enforced. Unknown keys are silently ignored.
- `apply_server_properties` and `parse_config_over` public functions: allow callers to build a config from properties then apply CLI overrides. Precedence: CLI flag → `server.properties` → built-in defaults.
- `ServerConfig` now carries `motd`, `max_players`, and `online_mode` fields from properties.
- Startup log now prints MOTD, max players, online-mode status, and bind address at launch.
- 47 new tests (22 in `properties` module, 15 `server.properties`/config integration tests in `aurelia-server`).

### Changed

- `./aurelia-server` now works without any arguments: binds to `0.0.0.0:25565`, reads `server.properties`, auto-detects world format, and creates a flat world if none exists. No `--experimental-join`, `--playable-flat-world`, or other flags required for normal use.
- `ServerConfig::default_config()` now enables join by default (`experimental_join_enabled: true`, `playable_flat_world: true`). Debug flag constructors (`ServerConfig::new`, `with_options`) still default to join-disabled for test isolation.
- README Quick Start updated to document `./aurelia-server` as the normal launch command with server.properties table and CLI flag reference.

### Fixed

- Block placement handler: removed `expect("validated inventory")` panic path; replaced with a safe early return (the `expect` was sound but exposed an unnecessary panic site).
- `server.properties` added to `.gitignore` as a runtime-generated file.

## 0.2.0 - Vanilla Parity Foundation (2026-06-11)

### Added

- Experimental real-client flat-world join path for Minecraft Beta 1.7.3 protocol `14`.
- Basic movement handling and chunk streaming with load/unload visibility updates across chunk boundaries.
- Conservative inventory sync and player inventory window click handling.
- Starter hotbar, held-slot tracking, and inventory-backed block placement.
- Complete Beta 1.7.3 rule-table coverage: every block id (`0..=96`) and item id (`256..=357` plus music discs `2256`/`2257`) with Beta-era stack sizes, materials, hardness, tool/tier harvest requirements, drops, solidity, and light emission. Values from public documentation are isolated and marked approximate.
- Item taxonomy for hoes, gold-tier tools (harvesting like wood), armor slots, and technical block forms that can never be placed from an inventory stack (fluids, fire, door/sign/bed/repeater block forms, piston internals, portal).
- Chat echo and debug commands including `/aurelia`, `/whereami`, `/givebasic`, `/save`, `/setblock`, and `/time`.
- Aurelia-native dirty flat-world chunk persistence.
- Aurelia-native player persistence for username, position, rotation, health, selected hotbar slot, inventory, and spawn position.
- Server-side health/death/fall/void foundations without unverified client-visible packet claims.
- Vanilla parity matrix and compatibility tracking docs.
- Public repository polish: README rewrite, contribution docs, community files, GitHub issue templates, funding metadata, CI workflow, release checklist, and repository setup helpers.

### Fixed

- Legacy string decoding now rejects lengths above the hard maximum before reading the payload, closing a per-packet oversized-read hole in chat decoding.
- Player save file names now use an injective escape for unusual username characters, so two distinct usernames can never collide on the same save file.
- Player registration and saved-state loading now happen under a single game-state lock during login.
- Time-update and keepalive enabled flags can no longer drift apart from their modes; consultation goes through single-source helpers.
- Diamond ore now drops diamond instead of itself; redstone ore now drops redstone dust instead of itself.

### Known Missing

- Verified production login response semantics.
- Crafting, chests, furnaces, workbench UI, full inventory rules, and smelting.
- Item entities, pickups, mobs, AI, combat, and visible entity packet parity.
- Vanilla terrain generation, worldgen features, fluids, redstone, weather, sleep, permissions, and vanilla commands.
- Vanilla McRegion/NBT world and player save parity.

### Notes

- `0.2.0` is an early compatibility foundation, not a complete Beta 1.7.3 server.
- Current persistence is Aurelia-native and not vanilla McRegion/NBT-compatible.
- Compatibility claims require clean-room evidence.
