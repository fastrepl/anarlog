import { Outlet, useNavigate } from "@tanstack/react-router";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { events as windowsEvents } from "@anlg/plugin-windows";

import {
  openNewNoteAndListen,
  openSessionAndListen,
  useNewNote,
} from "./useNewNote";

import { AuthProvider } from "~/auth";
import { BillingProvider } from "~/auth/billing";
import { DevtoolsFloatingPanelHost } from "~/devtools-panel/host";
import { getOrCreateSessionForEventId } from "~/session/queries";
import { useMyWorkspacesWithMirror } from "~/settings/team/mirror";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { UndoDeleteToast } from "~/sidebar/toast/undo-delete-toast";
import { isTabInputSupported, useTabs } from "~/store/zustand/tabs";

export default function MainAppLayout() {
  useNavigationEvents();

  return (
    <AuthProvider>
      <BillingProvider>
        <SharedWorkspaceMirror />
        <MainAppContent />
      </BillingProvider>
    </AuthProvider>
  );
}

function MainAppContent() {
  return (
    <>
      <Outlet />
      <UndoDeleteToast />
      <DevtoolsFloatingPanelHost />
    </>
  );
}

const useNavigationEvents = () => {
  const navigate = useNavigate();
  const openNew = useTabs((state) => state.openNew);
  const openNewNote = useNewNote({ behavior: "new" });

  useMountEffect(() => {
    const navigateFromNative = (path: string) => {
      const match = path.match(/^\/app\/([^/]+)\/(.+)$/);
      if (!match) return;
      const [, type, id] = match;
      if (type === "session") {
        openNew({ type: "sessions", id });
      } else if (type === "human") {
        openNew({
          type: "contacts",
          state: { selected: { type: "person", id } },
        });
      } else if (type === "organization") {
        openNew({
          type: "contacts",
          state: { selected: { type: "organization", id } },
        });
      }
    };
    (window as any).__ANARLOG_NAVIGATE__ = navigateFromNative;

    let unlistenNavigate: (() => void) | undefined;
    let unlistenOpenTab: (() => void) | undefined;
    let cancelled = false;

    const webview = getCurrentWebviewWindow();

    void windowsEvents
      .navigate(webview)
      .listen(({ payload }) => {
        if (payload.path === "/app/new") {
          const calendarEventId = payload.search?.calendarEventId;
          const shouldRecord = payload.search?.record === "true";

          if (typeof calendarEventId === "string" && calendarEventId) {
            void getOrCreateSessionForEventId(calendarEventId)
              .then((sessionId) => {
                if (shouldRecord) {
                  openSessionAndListen(sessionId, { behavior: "new" });
                  return;
                }

                openNew({
                  type: "sessions",
                  id: sessionId,
                  state: {
                    view: null,
                    autoStart: null,
                  },
                });
              })
              .catch((error) => {
                console.error(
                  "[navigation] failed to open calendar event",
                  error,
                );
              });
          } else if (shouldRecord) {
            openNewNoteAndListen({ behavior: "new" });
          } else {
            openNewNote();
          }
        } else if (payload.path === "/app/settings") {
          const tab = (payload.search?.tab as string) ?? "app";
          openNew({ type: "settings", state: { tab } });
        } else {
          void navigate({
            to: payload.path,
            search: payload.search ?? undefined,
          });
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlistenNavigate = fn;
        }
      });

    void windowsEvents
      .openTab(webview)
      .listen(({ payload }) => {
        if (payload.tab.type === "sessions" && payload.tab.id === "new") {
          openNewNote();
        } else if (!isTabInputSupported(payload.tab)) {
          return;
        } else {
          openNew(payload.tab);
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlistenOpenTab = fn;
        }
      });

    return () => {
      cancelled = true;
      if ((window as any).__ANARLOG_NAVIGATE__ === navigateFromNative) {
        delete (window as any).__ANARLOG_NAVIGATE__;
      }
      unlistenNavigate?.();
      unlistenOpenTab?.();
    };
  });
};

// Renders nothing; keeps the local workspace mirror fresh so sharing scopes are
// available without visiting Team settings.
function SharedWorkspaceMirror() {
  useMyWorkspacesWithMirror();
  return null;
}
