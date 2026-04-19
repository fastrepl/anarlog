# Invariant

- `crates/calendar-sync` is a pure sync engine. Do not add Tauri, auth, HTTP/provider fetching, or plugin record-schema details here.
- The crate may own scheduler, sync plans, minimal `Incoming*` types, and narrow traits over fetch/store/runtime boundaries.
- Normalization of provider data and concrete persistence formats belong in the plugin.
