import { DatabaseIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { execute } from "@hypr/plugin-reactive-db";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useLiveQuery } from "./use-live-query";

import { StandardTabWrapper } from "~/shared/main";
import { TabItemBase, type TabItem } from "~/shared/tabs";
import type { Tab } from "~/store/zustand/tabs";

type SampleTab = Extract<Tab, { type: "sample" }>;

export const TabItemSample: TabItem<SampleTab> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => (
  <TabItemBase
    icon={<DatabaseIcon className="h-4 w-4" />}
    title="Sample"
    selected={tab.active}
    pinned={tab.pinned}
    tabIndex={tabIndex}
    handleCloseThis={() => handleCloseThis(tab)}
    handleSelectThis={() => handleSelectThis(tab)}
    handleCloseOthers={handleCloseOthers}
    handleCloseAll={handleCloseAll}
    handlePinThis={() => handlePinThis(tab)}
    handleUnpinThis={() => handleUnpinThis(tab)}
  />
);

export function TabContentSample({ tab }: { tab: SampleTab }) {
  return (
    <StandardTabWrapper>
      <SampleView />
    </StandardTabWrapper>
  );
}

interface Session {
  id: string;
  title: string;
  created_at: string;
}

function SampleView() {
  const [title, setTitle] = useState("");
  const [ready, setReady] = useState(false);

  useEffect(() => {
    execute(
      `CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
      )`,
    ).then(() => setReady(true));
  }, []);

  const { data: sessions, isLoading } = useLiveQuery<Session>(
    "SELECT id, title, created_at FROM sessions ORDER BY created_at DESC",
    [],
    { enabled: ready },
  );

  const { data: countResult } = useLiveQuery<{ count: number }>(
    "SELECT COUNT(*) as count FROM sessions",
    [],
    { enabled: ready },
  );

  const count = countResult?.[0]?.count ?? 0;

  const handleCreate = useCallback(async () => {
    const trimmed = title.trim();
    if (!trimmed) return;

    await execute(
      "INSERT INTO sessions (id, title, created_at) VALUES (?, ?, datetime('now'))",
      [crypto.randomUUID(), trimmed],
    );
    setTitle("");
  }, [title]);

  return (
    <div className="flex h-full flex-col">
      <div
        className={cn([
          "flex items-center gap-2 border-b border-neutral-200 px-4 py-3",
        ])}
      >
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleCreate();
          }}
          placeholder="New session title..."
          className="flex-1 rounded-md border border-neutral-300 px-3 py-1.5 text-sm outline-none focus:border-blue-400"
        />
        <Button size="sm" onClick={handleCreate} disabled={!title.trim()}>
          Create
        </Button>
        <span className="text-xs text-neutral-500">
          {count} session{count !== 1 ? "s" : ""}
        </span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="p-4 text-sm text-neutral-400">Loading...</div>
        ) : sessions?.length === 0 ? (
          <div className="p-4 text-sm text-neutral-400">
            No sessions yet. Create one above.
          </div>
        ) : (
          <ul className="divide-y divide-neutral-100">
            {sessions?.map((session) => (
              <li
                key={session.id}
                className="flex items-center justify-between px-4 py-2"
              >
                <span className="text-sm">{session.title}</span>
                <span className="text-xs text-neutral-400">
                  {session.created_at}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
