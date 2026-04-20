import { randomUUID } from "node:crypto";

import type { QueryEvent } from "@hypr/db-runtime";
import * as sdk from "@hypr/napi-sdk";

import type { DbSubscribeResult } from "../shared/subscribe.js";

// Ref-counted bridge between `sdk.subscribe(sql, params, cb)` and renderer
// listeners. Mirrors `plugins/db/src/runtime.rs::PluginDbRuntime` on the
// Tauri side — generic SQL + params, no per-entity query identity.
//
// Invariants (see `subscription-architecture.md`):
//   1. One NAPI handle per `(sql, params)` key, shared across all renderer
//      listeners of that key. Starting is idempotent.
//   2. Each `start()` mints a private renderer channel
//      (`hypr:db:subscribe:delta:<uuid>`) and records the subscribing
//      `WebContents`. Deltas go only to that channel — never to
//      `BrowserWindow.getAllWindows()`.
//   3. `WebContents.once("destroyed", …)` cascades into `stop()` for every
//      subscription the window owned, tearing down the NAPI handle once the
//      query key's refcount hits zero.
//   4. `send` failures are treated as dead subscribers and cleaned up.

type LiveQueryState = {
  handle: sdk.SubscriptionHandle;
  reactive: boolean;
  subscriptionIds: Set<string>;
};

type RendererSubscription = {
  id: string;
  queryKey: string;
  sender: Electron.WebContents;
  senderId: number;
  channel: string;
};

export class LiveQuerySubscriptionManager {
  private readonly activeQueries = new Map<string, LiveQueryState>();
  private readonly activeSubscriptions = new Map<
    string,
    RendererSubscription
  >();
  private readonly subscriptionsByChannel = new Map<string, string>();
  private readonly subscriptionsBySenderId = new Map<number, Set<string>>();

  start(
    sql: string,
    params: unknown[],
    sender: Electron.WebContents,
  ): DbSubscribeResult {
    const queryKey = buildLiveQueryKey(sql, params);
    const state = this.ensureQueryHandle(sql, params, queryKey);

    const subscriptionId = randomUUID();
    const channel = `hypr:db:subscribe:delta:${randomUUID()}`;

    this.activeSubscriptions.set(subscriptionId, {
      id: subscriptionId,
      queryKey,
      sender,
      senderId: sender.id,
      channel,
    });
    this.subscriptionsByChannel.set(channel, subscriptionId);
    state.subscriptionIds.add(subscriptionId);
    this.trackSenderLifetime(sender);
    this.addSubscriptionForSender(sender.id, subscriptionId);

    return { channel, reactive: state.reactive };
  }

  // Renderer-initiated teardown. The renderer only holds the private
  // channel name it received from `start()`.
  stopByChannel(channel: string): boolean {
    const subscriptionId = this.subscriptionsByChannel.get(channel);
    if (!subscriptionId) return false;
    return this.stop(subscriptionId);
  }

  private stop(subscriptionId: string): boolean {
    const subscription = this.activeSubscriptions.get(subscriptionId);
    if (!subscription) return false;

    this.activeSubscriptions.delete(subscriptionId);
    this.subscriptionsByChannel.delete(subscription.channel);

    const senderIds = this.subscriptionsBySenderId.get(subscription.senderId);
    if (senderIds) {
      senderIds.delete(subscriptionId);
      if (senderIds.size === 0) {
        this.subscriptionsBySenderId.delete(subscription.senderId);
      }
    }

    const state = this.activeQueries.get(subscription.queryKey);
    if (state) {
      state.subscriptionIds.delete(subscriptionId);
      if (state.subscriptionIds.size === 0) {
        try {
          state.handle.unsubscribe();
        } catch (error) {
          console.warn(
            `[desktop2] failed to unsubscribe live query ${subscription.queryKey}`,
            error,
          );
        }
        this.activeQueries.delete(subscription.queryKey);
      }
    }

    return true;
  }

  destroy(): void {
    for (const state of this.activeQueries.values()) {
      try {
        state.handle.unsubscribe();
      } catch (error) {
        console.warn("[desktop2] failed to unsubscribe during destroy", error);
      }
    }
    this.activeQueries.clear();
    this.activeSubscriptions.clear();
    this.subscriptionsByChannel.clear();
    this.subscriptionsBySenderId.clear();
  }

  private ensureQueryHandle(
    sql: string,
    params: unknown[],
    queryKey: string,
  ): LiveQueryState {
    const existing = this.activeQueries.get(queryKey);
    if (existing) return existing;

    const handle = sdk.subscribe(sql, params as never[], (delta) => {
      this.forwardToRenderers(queryKey, normalizeSdkDelta(delta));
    });
    const state: LiveQueryState = {
      handle,
      reactive: handle.reactive,
      subscriptionIds: new Set(),
    };
    this.activeQueries.set(queryKey, state);
    return state;
  }

  private forwardToRenderers(queryKey: string, event: QueryEvent): void {
    const state = this.activeQueries.get(queryKey);
    if (!state) return;

    const dead: string[] = [];
    for (const subscriptionId of state.subscriptionIds) {
      const subscription = this.activeSubscriptions.get(subscriptionId);
      if (!subscription || subscription.sender.isDestroyed()) {
        dead.push(subscriptionId);
        continue;
      }

      try {
        subscription.sender.send(subscription.channel, event);
      } catch (error) {
        console.warn(
          `[desktop2] failed to forward live query ${queryKey} delta to renderer`,
          error,
        );
        dead.push(subscriptionId);
      }
    }

    for (const id of dead) this.stop(id);
  }

  private trackSenderLifetime(sender: Electron.WebContents): void {
    if (this.subscriptionsBySenderId.has(sender.id)) return;

    this.subscriptionsBySenderId.set(sender.id, new Set());
    const senderId = sender.id;
    sender.once("destroyed", () => {
      const ids = this.subscriptionsBySenderId.get(senderId);
      if (!ids) return;
      for (const subscriptionId of Array.from(ids)) this.stop(subscriptionId);
      this.subscriptionsBySenderId.delete(senderId);
    });
  }

  private addSubscriptionForSender(
    senderId: number,
    subscriptionId: string,
  ): void {
    const ids = this.subscriptionsBySenderId.get(senderId);
    if (!ids) {
      this.subscriptionsBySenderId.set(senderId, new Set([subscriptionId]));
      return;
    }
    ids.add(subscriptionId);
  }
}

function buildLiveQueryKey(sql: string, params: unknown[]): string {
  return `${sql}::${JSON.stringify(params)}`;
}

// Translate SDK deltas into the `QueryEvent` shape `@hypr/db-runtime`
// expects, matching what `plugins/db` emits on the Tauri side. `reactive`
// is preserved on the start-time handle, not per-delta.
function normalizeSdkDelta(delta: sdk.LiveQueryDelta): QueryEvent {
  if (delta.event === "snapshot") {
    return { event: "result", data: delta.rows ?? [] };
  }

  return {
    event: "error",
    data: delta.error ?? "Unknown live query subscription error",
  };
}
