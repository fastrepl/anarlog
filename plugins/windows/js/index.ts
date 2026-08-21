import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export * from "./bindings.gen";
import { commands, events } from "./bindings.gen";

export type WindowLabel =
  | "main"
  | "composer"
  | "floating"
  | "floating-bar"
  | "live-caption"
  | `note-${string}`
  | "calendar"
  | "settings";

export const getCurrentWebviewWindowLabel = () => {
  const window = getCurrentWebviewWindow();
  return window.label as WindowLabel;
};

export async function openUrlWithInstruction(
  url: string,
  instructionType: string,
  openUrl: (
    url: string,
  ) => Promise<{ status: "ok" | "error"; error?: unknown }>,
  instructionSearch?: Record<string, string | undefined>,
) {
  await commands.windowSaveFrame({ type: "main" });
  const search = Object.fromEntries(
    Object.entries({ type: instructionType, url, ...instructionSearch }).filter(
      ([, value]) => value !== undefined,
    ),
  );
  await commands.windowEmitNavigate(
    { type: "main" },
    { path: "/app/instruction", search },
  );
  await commands.windowSetFrameAnimated({ type: "main" }, "TopRight", 340, 500);

  try {
    const result = await openUrl(url);
    if (result.status === "error") {
      throw new Error(String(result.error));
    }
  } catch (error) {
    await commands.windowEmitNavigate(
      { type: "main" },
      { path: "/app", search: null },
    );
    await commands.windowRestoreFrameAnimated({ type: "main" });
    throw error;
  }
}

export async function dismissInstruction() {
  await commands.windowEmitNavigate(
    { type: "main" },
    { path: "/app", search: null },
  );
  await new Promise((resolve) => setTimeout(resolve, 100));
  await commands.windowRestoreFrameAnimated({ type: "main" });
}

const DROP_PREVENTION_CLEANUP_KEY =
  "__ANARLOG_WINDOWS_DROP_PREVENTION_CLEANUP__";
const HEALTH_CHECK_CLEANUP_KEY = "__ANARLOG_WINDOWS_HEALTH_CHECK_CLEANUP__";

export const init = () => {
  const host = window as typeof window & {
    [DROP_PREVENTION_CLEANUP_KEY]?: () => void;
    [HEALTH_CHECK_CLEANUP_KEY]?: () => void;
  };
  host[DROP_PREVENTION_CLEANUP_KEY]?.();
  host[HEALTH_CHECK_CLEANUP_KEY]?.();

  const allowDropAttribute = "[data-allow-file-drop='true']";
  const shouldAllow = (event: DragEvent) => {
    if (!(event.target instanceof Element)) {
      return false;
    }
    return Boolean(event.target.closest(allowDropAttribute));
  };

  const preventUnlessAllowed = (event: DragEvent) => {
    const allowed = shouldAllow(event);

    if (event.type === "dragover" || event.type === "drop") {
      // Always prevent the browser's default "open file in this window"
      // behaviour so the webview doesn't navigate away.
      event.preventDefault();

      // Outside allowed regions we also stop propagation so nothing else
      // (including Tiptap) sees the event.
      if (!allowed) {
        event.stopPropagation();
      }
      return;
    }

    // For dragenter / dragleave we only block events outside allowed areas.
    if (!allowed) {
      event.preventDefault();
      event.stopPropagation();
    }
  };

  document.addEventListener("dragover", preventUnlessAllowed);
  document.addEventListener("drop", preventUnlessAllowed);
  document.addEventListener("dragenter", preventUnlessAllowed);
  document.addEventListener("dragleave", preventUnlessAllowed);

  const cleanupDropPrevention = () => {
    document.removeEventListener("dragover", preventUnlessAllowed);
    document.removeEventListener("drop", preventUnlessAllowed);
    document.removeEventListener("dragenter", preventUnlessAllowed);
    document.removeEventListener("dragleave", preventUnlessAllowed);
    if (host[DROP_PREVENTION_CLEANUP_KEY] === cleanupDropPrevention) {
      delete host[DROP_PREVENTION_CLEANUP_KEY];
    }
  };
  host[DROP_PREVENTION_CLEANUP_KEY] = cleanupDropPrevention;

  let healthCheckUnlisten: (() => void) | null = null;
  let healthCheckCancelled = false;
  void events.webviewHealthCheck
    .listen(({ payload }) => {
      void commands.webviewHealthAck(payload.requestId);
    })
    .then((unlisten) => {
      if (healthCheckCancelled) {
        unlisten();
      } else {
        healthCheckUnlisten = unlisten;
        void commands.webviewHealthReady().catch((error) => {
          console.error("Failed to mark webview health checks ready", error);
        });
      }
    })
    .catch((error) => {
      console.error("Failed to listen for webview health checks", error);
    });

  const cleanupHealthCheck = () => {
    healthCheckCancelled = true;
    healthCheckUnlisten?.();
    if (host[HEALTH_CHECK_CLEANUP_KEY] === cleanupHealthCheck) {
      delete host[HEALTH_CHECK_CLEANUP_KEY];
    }
  };
  host[HEALTH_CHECK_CLEANUP_KEY] = cleanupHealthCheck;

  return () => {
    cleanupDropPrevention();
    cleanupHealthCheck();
  };
};
