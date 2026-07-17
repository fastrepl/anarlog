---
name: axum-test-transport
description: Choose between in-process `tower::ServiceExt::oneshot` tests and real `tokio::net::TcpListener` server tests for Axum services in this repository.
metadata:
  internal: true
---

Use this skill when working on Axum tests that currently start a server or could be simplified.

Prefer `oneshot` when the test only verifies:

- status codes
- headers
- JSON/body shape
- route or middleware behavior
- request handling without real network behavior

Use:

```rust
use tower::ServiceExt;
```

`oneshot` is available through `tower::ServiceExt`.

Keep a real `TcpListener` + `axum::serve(...)` when the test needs:

- WebSocket upgrade/handshake behavior
- real client behavior (`reqwest`, `tokio_tungstenite`, etc.) as the thing under test
- transport/lifecycle concerns like bind/listen/shutdown
- true end-to-end coverage over `http://` or `ws://`

Repository guidance:

- `crates/transcribe-cactus/tests/batch.rs` style tests are good `oneshot` candidates
- `crates/transcribe-cactus/tests/live.rs` WebSocket tests should stay real-server
- if several tests share the same in-process request setup, add a tiny local helper (for example request builder or response JSON parser)

After changes:

- run `pnpm exec dprint fmt`
- run the relevant Rust test compile/check command
