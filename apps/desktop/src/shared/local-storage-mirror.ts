import { commands } from "~/types/tauri.gen";

// The native app that replaces this one (apps/desktop-gpui) can't read this web
// view's localStorage, so it is mirrored to a file that app imports; the Rust
// side keeps only the keys it has a place for
// (crates/desktop-identity/src/webview_storage.rs). Writes are batched once a
// second: a burst of changes, such as a Cloud API backfill, writes the file
// once instead of once per meeting.
const MIRROR_DELAY_MS = 1_000;

let timer: ReturnType<typeof setTimeout> | null = null;
let writing: Promise<void> = Promise.resolve();

export function mirrorLocalStorage(): void {
  timer ??= setTimeout(() => {
    timer = null;
    void write();
  }, MIRROR_DELAY_MS);
}

export function flushLocalStorageMirror(): Promise<void> {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  return write();
}

// The main window mirrors at startup and after another window's writes, so
// the file ends with the latest entries even when two windows' writes race.
export function startLocalStorageMirror(): void {
  window.addEventListener("storage", mirrorLocalStorage);
  mirrorLocalStorage();
}

// One write at a time, each reading the entries when it starts, so an older
// snapshot never lands after a newer one.
function write(): Promise<void> {
  writing = writing.then(send);
  return writing;
}

async function send(): Promise<void> {
  try {
    const result = await commands.mirrorLocalStorage(readEntries());
    if (result.status === "error") {
      console.warn("[local-storage-mirror] write failed", result.error);
    }
  } catch (error) {
    console.warn("[local-storage-mirror] write failed", error);
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
