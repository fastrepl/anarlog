# Subscription architecture

Why the Rust → renderer delta pipeline is structured the way it is, and what
invariants `LiveQuerySubscriptionManager` preserves.

## Goal

Each renderer that subscribes to a live SQL query sees deltas for that
specific `(sql, params)` pair, and only for it. Closing a window must stop
its deltas. A closed window must not prevent the next window from
subscribing. The Rust handle for a query key must exist exactly while at
least one renderer is listening.

## Why subscription state lives in Node, not Rust

The Rust surface (`crates/hypr-api::live::subscribe`) exposes a minimal
primitive: it returns a `SubscriptionHandle` with a single-shot reactivity
analysis. Rust does not know about:

- `WebContents`, window lifecycle, or renderer process crashes.
- IPC channel allocation or fanout policy.
- Whether a given subscription is still reachable.

Keeping those concerns in Node matches where the relevant runtime state
already lives (`BrowserWindow`, `webContents.id`, `ipcMain` /
`webContents.send`). Matches `apps/lite/electron/src/watcher-architecture.md`
in `gitbutlerapp/gitbutler`; our adaptation keys on `(sql, JSON.stringify(
params))` instead of project ids.

## Invariants

1. **One NAPI handle per query key, regardless of subscriber count.** If
   two windows subscribe with the same `(sql, params)`, Rust sees one
   handle, not two. `ensureQueryHandle` tracks handles in `activeQueries`;
   starts are idempotent.

2. **Deltas fan out per-subscriber, not per-window-list.** Each `start()`
   call mints a private channel `hypr:db:subscribe:delta:<uuid>`; deltas
   go to that channel's `WebContents` only. `BrowserWindow.getAllWindows()`
   is never used. Even today (a window that didn't subscribe receives
   nothing) and more so once the app supports multiple windows.

3. **Closing a window cleans up its subscriptions automatically.**
   `trackSenderLifetime` registers `webContents.once("destroyed", …)` on
   first subscription for a sender. When the window closes, every
   subscription it owned is `stop()`-ed, which transitively frees the NAPI
   handle once the query key's refcount hits zero.

4. **Dead subscribers self-heal during forwarding.** If
   `sender.isDestroyed()` or `send` throws while forwarding a delta, the
   subscription is queued for removal after the fanout loop. This catches
   subscribers that died between `destroyed`-event registration and the
   next delta.

5. **`destroy()` is safe to call on shutdown.** It unsubscribes every live
   NAPI handle, clears maps, and is tolerant of individual failures.

## Query key vs. subscription lifecycle

Two different things. Keeping them distinct matters.

- A **query key** (`${sql}::${JSON.stringify(params)}`) has at most one NAPI
  handle, held in `activeQueries`. Its lifetime is "ref-counted by
  subscriptions". `reactive` is captured once per handle from the Rust
  analysis.
- A **subscription** is a single renderer-side listener. It has an id, a
  private channel, and points at one query key. Its lifetime is "from
  `start()` to `stop()`, or until the owning `WebContents` is destroyed".

Query keys are deduplicated. Subscriptions are not: two `start(sql,
params)` calls from the same window mint two subscriptions with two
channels. In practice the renderer layer is expected to hold at most one
subscription per query key per window (drizzle `useDrizzleLiveQuery` does
this naturally), but the manager does not enforce it.

## Wire format

Main emits `QueryEvent` (same shape as `@hypr/db-runtime`):

```ts
type QueryEvent<T = Row> =
  | { event: "result"; data: T[] }
  | { event: "error"; data: string };
```

Rows are raw positional/named values as returned by `hypr_db_execute`.
Typing them is the renderer's job — drizzle handles that via the proxy
driver against `executeProxy`, and `subscribe` typing flows through
`LiveQueryClient.subscribe<T>`.

`reactive` is returned once from `hypr:db:subscribe` (via
`DbSubscribeResult`), not per-delta. Preload logs a warning once if a
query is non-reactive.
