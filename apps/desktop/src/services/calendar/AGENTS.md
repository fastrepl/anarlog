# Calendar Reconcile Policy

- `sessions.event_json` and auto participant mappings are authoritative only for successfully observed, in-horizon calendar events.
- In the current TinyBase bridge, `calendar-sync` keeps out-of-scope events in the cache. For a session-linked `tracking_id`, a missing cached row therefore means a positive delete, not "unknown".
- Positive deletes include provider removal, disabled calendars, and disconnected accounts: clear `sessions.event_json` and remove `source: "auto"` participant mappings.
- `source: "excluded"` participant mappings stay sticky.
- If future Rust/SQLite work starts evicting out-of-scope rows, do not infer deletes from cache absence. Move this policy to an explicit `Observed` / `Deleted` / `Untouched` sync outcome instead.
