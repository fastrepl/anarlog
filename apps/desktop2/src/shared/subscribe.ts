// Response from the `hypr:db:subscribe` IPC call. `channel` is a private
// per-subscription ipcRenderer channel name that preload listens on for
// `QueryEvent` deltas. `reactive` is the one-time analysis result reported
// by `hypr_db_reactive::SubscriptionRegistration.analysis` — mirrored from
// the Tauri side, surfaced once on subscribe so the renderer can decide
// whether to warn the developer about non-reactive SQL.
export type DbSubscribeResult = {
  channel: string;
  reactive: boolean;
};
