import { listen } from "@tauri-apps/api/event";

import { commands } from "~/types/tauri.gen";

// The native app that replaces this one (apps/desktop-gpui) can't read this web
// view's localStorage, so it is mirrored to a file that app imports; the Rust
// side keeps only the keys it has a place for
// (crates/desktop-identity/src/webview_storage.rs). Writes are batched once a
// second: a burst of changes, such as a Cloud API backfill, writes the file
// once instead of once per meeting.
const FLUSH_WEBVIEW_STORAGE_EVENT = "updater2:flush-webview-storage";
const MIRROR_DELAY_MS = 1_000;

let timer: ReturnType<typeof setTimeout> | null = null;
let writing: Promise<void> = Promise.resolve();

export function mirrorLocalStorage(): void {
  timer ??= setTimeout(() => {
    timer = null;
    void write(false);
  }, MIRROR_DELAY_MS);
}

export function flushLocalStorageMirror(): Promise<void> {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  return write(true);
}

function flushBestEffort(): Promise<void> {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  return write(false);
}

// The main window mirrors at startup and after another window's writes, so
// the file ends with the latest entries even when two windows' writes race.
export function startLocalStorageMirror(): void {
  window.addEventListener("storage", mirrorLocalStorage);
  void listen(FLUSH_WEBVIEW_STORAGE_EVENT, () => {
    void flushBestEffort();
  })
    .catch((error) => {
      console.warn("[local-storage-mirror] flush listener failed", error);
    })
    .finally(() => {
      void write(false);
    });
}

// One write at a time, each reading the entries when it starts, so an older
// snapshot never lands after a newer one. Keeping `writing` settled lets later
// writes run even after a strict flush fails.
function write(strict: boolean): Promise<void> {
  const next = writing.then(() => send(strict));
  writing = next.catch(() => {});
  return next;
}

async function send(strict: boolean): Promise<void> {
  try {
    const result = await commands.mirrorLocalStorage(readEntries());
    if (result.status === "error") {
      throw new Error(result.error);
    }
  } catch (error) {
    console.warn("[local-storage-mirror] write failed", error);
    if (strict) {
      throw error;
    }
  }
}

function readEntries(): Record<string, string> {
  const entries: Record<string, string> = {};
  for (let index = 0; index < localStorage.length; index += 1) {
    const key = localStorage.key(index);
    const value = key === null ? null : localStorage.getItem(key);
    if (key !== null && value !== null) {
      entries[key] = value;
    }
  }
  return entries;
}
