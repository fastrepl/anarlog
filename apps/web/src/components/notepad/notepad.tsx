import { Link } from "@tanstack/react-router";
import type { LucideIcon } from "lucide-react";
import { createContext, useContext, useMemo } from "react";
import { Provider as TinyBaseProvider } from "tinybase/ui-react";

import { cn } from "@hypr/utils";

import { TranscriptionHero } from "@/components/transcription/hero";
import { STORE_ID, StoreProvider, UI } from "@/store/tinybase";

export type TabMeta = {
  value: string;
  icon: LucideIcon;
  label?: string;
  title?: string;
  pinned?: boolean;
  sessionId?: string;
  content: React.ReactNode;
};

type NotepadContextValue = {
  activeTab: string | undefined;
  searchId: string | undefined;
};

const NotepadContext = createContext<NotepadContextValue | null>(null);

function useNotepadContext() {
  const ctx = useContext(NotepadContext);
  if (!ctx) throw new Error("useNotepadContext must be used inside <Notepad>");
  return ctx;
}

export function useNotepadSearch() {
  const ctx = useNotepadContext();
  return { searchId: ctx.searchId };
}

function SessionTabLabel({
  sessionId,
  initialTitle,
}: {
  sessionId: string;
  initialTitle?: string;
}) {
  const title = UI.useCell("sessions", sessionId, "title", STORE_ID) as string;
  return <>{title || initialTitle || "Untitled"}</>;
}

function TabBar({
  tabs,
  activeTab,
  searchId,
}: {
  tabs: TabMeta[];
  activeTab: string | undefined;
  searchId: string | undefined;
}) {
  return (
    <div className="flex h-full items-center gap-1 px-1 py-1">
      {tabs.map((tc) => {
        const selected = activeTab === tc.value;
        const Icon = tc.icon;

        return (
          <Link
            key={tc.value}
            to="/app/file-transcription/"
            search={{ tab: tc.value, id: searchId }}
            className={cn([
              "relative flex items-center gap-2",
              "h-full w-[160px] px-2",
              "rounded-xl border",
              "cursor-pointer",
              "transition-colors duration-200",
              selected
                ? ["bg-neutral-50", "text-black", "border-stone-300"]
                : [
                    "bg-transparent",
                    "text-neutral-500",
                    "border-transparent",
                    "hover:text-neutral-700",
                  ],
            ])}
          >
            <div className="flex h-4 w-4 shrink-0 items-center justify-center">
              <Icon size={14} />
            </div>
            <span className="truncate text-sm">
              {tc.sessionId ? (
                <SessionTabLabel
                  sessionId={tc.sessionId}
                  initialTitle={tc.title}
                />
              ) : (
                tc.label
              )}
            </span>
          </Link>
        );
      })}
    </div>
  );
}

function WindowShell({
  tabs,
  activeTab,
  searchId,
  title,
  children,
}: {
  tabs: TabMeta[];
  activeTab: string | undefined;
  searchId: string | undefined;
  title?: string;
  children: React.ReactNode;
}) {
  const showTabBar = tabs.length > 1;

  return (
    <div className="relative min-h-[calc(100vh-280px)] overflow-hidden rounded-xl border border-neutral-200">
      <div className="flex h-11 items-center border-b border-neutral-200 bg-neutral-100/60">
        <div className="flex shrink-0 items-center gap-2 px-4">
          <div className="size-3 rounded-full border border-black/10 bg-[#ff5f57]" />
          <div className="size-3 rounded-full border border-black/10 bg-[#ffbd2e]" />
          <div className="size-3 rounded-full border border-black/10 bg-[#28c840]" />
        </div>

        {showTabBar ? (
          <TabBar tabs={tabs} activeTab={activeTab} searchId={searchId} />
        ) : (
          title && (
            <span className="-ml-[52px] flex-1 text-center text-xs font-medium text-neutral-400">
              {title}
            </span>
          )
        )}
      </div>

      {children}
    </div>
  );
}

export function Notepad({
  searchId,
  activeTab,
  title,
  store = true,
  tabs,
}: {
  searchId?: string;
  activeTab?: string;
  title?: string;
  store?: boolean;
  tabs: TabMeta[];
}) {
  const resolvedActive = activeTab ?? tabs[0]?.value;

  const ctxValue = useMemo<NotepadContextValue>(
    () => ({ activeTab: resolvedActive, searchId }),
    [resolvedActive, searchId],
  );

  const activeContent =
    tabs.find((t) => t.value === resolvedActive)?.content ?? null;

  const content = (
    <NotepadContext.Provider value={ctxValue}>
      <div className="mx-auto max-w-3xl px-6 pb-10">
        <TranscriptionHero />
        <WindowShell
          tabs={tabs}
          activeTab={resolvedActive}
          searchId={searchId}
          title={title}
        >
          {activeContent}
        </WindowShell>
      </div>
    </NotepadContext.Provider>
  );

  if (!store) return content;

  return (
    <TinyBaseProvider>
      <StoreProvider>{content}</StoreProvider>
    </TinyBaseProvider>
  );
}
