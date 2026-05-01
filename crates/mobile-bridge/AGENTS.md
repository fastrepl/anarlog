# `mobile-bridge`

## Use This Crate For

- UniFFI transport over the app database for mobile callers that need synchronous host-facing methods backed by an internal Tokio runtime.

## Put Changes Elsewhere When

- Schema, migrations, and CloudSync table policy belong in `db-app`.
- One-shot SQL semantics belong in `db-execute`.
- Live-query invalidation rules belong in `db-reactive`.
- Desktop transport concerns belong in `plugins/db`.

## Hard Rules

- Keep semantic parity with `plugins/db`: same schema bootstrap, same execution behavior, same live-query contract, different transport only.
- The public ABI is string/JSON based. `params_json` must decode to an array, execute methods return JSON strings, and listener callbacks receive serialized rows or error strings. Treat shape changes as breaking API changes.
- Never invoke host callbacks while holding `self.state`. Reentrant listener calls back into the bridge are supported and tested.
- `open()` owns runtime creation and DB bootstrap. `close()` must unsubscribe active listeners, stop CloudSync, close the pool, and make future calls return `Closed`.
- `close()` is intentionally idempotent, and `Drop` delegates to it. Preserve that shutdown model.
- Every new subscription path must register its id in `subscription_ids`, and every teardown path must remove it, or shutdown will leak live subscriptions.
- Non-reactive subscriptions still deliver the initial result or error and only log the downgrade. Do not silently drop them.
- CloudSync config/status methods are transport shims. Keep JSON parsing/serialization and error mapping predictable for Swift/Kotlin callers.
