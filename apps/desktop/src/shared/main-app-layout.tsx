import { Outlet, useNavigate } from "@tanstack/react-router";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";

import { events as windowsEvents } from "@hypr/plugin-windows";

import { useNewNote } from "./main/useNewNote";

import { AuthProvider } from "~/auth";
import { BillingProvider } from "~/auth/billing";
import { NetworkProvider } from "~/contexts/network";
import {
  UndoDeleteKeyboardHandler,
  UndoDeleteToast,
} from "~/sidebar/toast/undo-delete-toast";
import { useTabs } from "~/store/zustand/tabs";

export default function MainAppLayout() {
  useNavigationEvents();

  return (
    <AuthProvider>
      <BillingProvider>
        <NetworkProvider>
          <MainAppContent />
        </NetworkProvider>
      </BillingProvider>
    </AuthProvider>
  );
}

function MainAppContent() {
  return (
    <>
      <Outlet />
      <UndoDeleteKeyboardHandler />
      <UndoDeleteToast />
    </>
  );
}

const useNavigationEvents = () => {
  const navigate = useNavigate();
  const openNew = useTabs((state) => state.openNew);
  const transitionChatMode = useTabs((state) => state.transitionChatMode);
  const openNewNote = useNewNote({ behavior: "new" });

  useEffect(() => {
    (window as any).__HYPR_NAVIGATE__ = (path: string) => {
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

    let unlistenNavigate: (() => void) | undefined;
    let unlistenOpenTab: (() => void) | undefined;

    const webview = getCurrentWebviewWindow();

    void windowsEvents
      .navigate(webview)
      .listen(({ payload }) => {
        if (payload.path === "/app/new") {
          openNewNote();
        } else if (payload.path === "/app/settings") {
          let tab = (payload.search?.tab as string) ?? "general";
          if (tab === "notifications" || tab === "account") {
            tab = "general";
          }
          if (tab === "calendar") {
            openNew({ type: "calendar" });
          } else if (tab === "transcription" || tab === "intelligence") {
            openNew({
              type: "ai",
              state: {
                tab: tab as "transcription" | "intelligence",
              },
            });
          } else {
            openNew({ type: "settings" });
          }
        } else {
          void navigate({
            to: payload.path,
            search: payload.search ?? undefined,
          });
        }
      })
      .then((fn) => {
        unlistenNavigate = fn;
      });

    void windowsEvents
      .openTab(webview)
      .listen(({ payload }) => {
        if (payload.tab.type === "sessions" && payload.tab.id === "new") {
          openNewNote();
        } else {
          openNew(payload.tab);
          if (payload.tab.type === "chat_support") {
            if (payload.tab.state) {
              const { tabs, updateChatSupportTabState } = useTabs.getState();
              const chatTab = tabs.find((t) => t.type === "chat_support");
              if (chatTab) {
                updateChatSupportTabState(chatTab, payload.tab.state);
              }
            }
            transitionChatMode({ type: "OPEN_TAB" });
          }
        }
      })
      .then((fn) => {
        unlistenOpenTab = fn;
      });

    return () => {
      delete (window as any).__HYPR_NAVIGATE__;
      unlistenNavigate?.();
      unlistenOpenTab?.();
    };
  }, [navigate, openNew, openNewNote, transitionChatMode]);
};
