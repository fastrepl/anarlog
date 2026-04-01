import type { Browser } from "wxt/browser";

import { type HostMessage, parseIncomingMessage } from "./shared/host-message";

const NATIVE_HOST_NAME = "com.char.native_host";

let nativePort: Browser.runtime.Port | null = null;

function connectNativeHost() {
  if (nativePort) {
    return nativePort;
  }

  nativePort = browser.runtime.connectNative(NATIVE_HOST_NAME);

  nativePort.onDisconnect.addListener(() => {
    const error = browser.runtime.lastError;
    if (error) {
      console.warn("[char] native host disconnected:", error.message);
    }
    nativePort = null;
  });

  return nativePort;
}

function postToNativeHost(payload: HostMessage) {
  try {
    const port = connectNativeHost();
    port.postMessage(payload);
    return true;
  } catch (error) {
    console.warn("[char] failed to post to native host:", error);
    nativePort = null;
    return false;
  }
}

export default defineBackground(() => {
  browser.runtime.onMessage.addListener((message) => {
    const payload = parseIncomingMessage(message);
    if (!payload) {
      return { ok: false, error: "invalid_message" };
    }

    const ok = postToNativeHost(payload);
    return { ok };
  });
});
