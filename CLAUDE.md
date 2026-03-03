# KiwiStack — Kiwi Mail

> **Layer:** Core
> **Upstream:** Stalwart Mail Server 0.10.5
> **License:** Apache-2.0 (Kiwi code) | AGPL-3.0 (Stalwart)

## Architecture

This component follows the [KiwiStack architecture](https://github.com/kiwistack/kiwi-stack/blob/main/architecture.md).

**Key principles:**
- One LXC container per component. Upstream binds to `127.0.0.1`, wrapper binds to `10.10.20.X`.
- The wrapper starts, health-checks, and proxies the upstream. Only the Kiwi API is exposed.
- All inter-service communication is HTTP/JSON over the private bridge. Auth via Kiwi ID JWT.
- Never link AGPL code. Never modify MPL files without sharing changes. All new code is Apache-2.0.

## LXC Model

```
┌──────────────────────────────────┐
│  LXC: kiwi-mail                  │
│                                  │
│  Upstream: 127.0.0.1:8080       │  ← Stalwart Mail Server
│       ↓ localhost                │
│  Wrapper: 10.10.10.111:8443     │  ← this crate
│                                  │
│  Only wrapper is on the network  │
└──────────────────────────────────┘
```

## Upstream Dependency

| Field | Value |
|-------|-------|
| Name | Stalwart Mail Server |
| Version | 0.10.5 |
| License | AGPL-3.0 |
| Protocol | JMAP (RFC 8620, RFC 8621) |
| Upstream port | `127.0.0.1:8080` |

**Constraints:**
- Do not modify upstream source code
- Do not link against upstream libraries (communicate over HTTP only)
- Pin the exact upstream version in `compatibility.toml`

## Build, Test, Run

```bash
# Build
cargo build

# Run all tests
cargo test

# Run the wrapper (development)
cargo run -p kiwi-mail

# Format check
cargo fmt --check

# Lint
cargo clippy -- -D warnings

# License check
cargo deny check licenses
```

## Workspace Layout

```
mail/
├── Cargo.toml              # Workspace root
├── CLAUDE.md               # This file
├── compatibility.toml      # Upstream version pin
├── crates/
│   ├── kiwi-mail/          # Main binary (wrapper)
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs
│   │       ├── upstream.rs
│   │       ├── jmap.rs
│   │       └── api/
│   │           ├── mod.rs
│   │           ├── health.rs
│   │           ├── mail.rs
│   │           └── tools.rs
│   ├── kiwi-mail-types/    # Shared types
│   └── kiwi-mail-client/   # Client library
├── scripts/
│   └── provision-lxc.sh
└── deny.toml
```
