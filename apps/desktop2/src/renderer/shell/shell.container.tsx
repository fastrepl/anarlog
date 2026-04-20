import { useMutation } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import { desc, sessions } from "@hypr/db";

import { isMac } from "~/bridge";
import { db, useDrizzleLiveQuery } from "~/db";
import { OpenNoteDialogContainer } from "~/open-note-dialog";
import { ProfileMenuContainer } from "~/profile-menu";
import { ShellView } from "~/shell/shell.view";
import {
  getStubTabLabel,
  selectCanGoBack,
  selectCanGoNext,
  selectCurrentTab,
  uniqueIdFromTab,
  useTabsShortcuts,
  useTabsStore,
} from "~/tabs";
import { UpdateBannerContainer } from "~/update-banner";

// Sessions query limited to fields we need for the tab strip. Using
// `.$inferSelect` gives us the whole row type; the explicit select narrows
// both payload and type.
const sessionsListQuery = db
  .select({
    id: sessions.id,
    title: sessions.title,
    updatedAt: sessions.updatedAt,
  })
  .from(sessions)
  .orderBy(desc(sessions.updatedAt), desc(sessions.id));

type SessionSummaryRow = {
  id: string;
  title: string;
  updatedAt: string;
};

export function ShellContainer({
  body,
  openNoteDialog,
}: {
  body: React.ReactNode;
  openNoteDialog?: React.ReactNode;
}) {
  const tabs = useTabsStore((state) => state.tabs);
  const currentTab = useTabsStore(selectCurrentTab);
  const openNew = useTabsStore((state) => state.openNew);
  const select = useTabsStore((state) => state.select);
  const close = useTabsStore((state) => state.close);
  const reorder = useTabsStore((state) => state.reorder);
  const clearSelection = useTabsStore((state) => state.clearSelection);
  const goBack = useTabsStore((state) => state.goBack);
  const goNext = useTabsStore((state) => state.goNext);
  const canGoBack = useTabsStore(selectCanGoBack);
  const canGoNext = useTabsStore(selectCanGoNext);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isChatOpen, setIsChatOpen] = useState(false);

  useTabsShortcuts();

  const sessionsQuery =
    useDrizzleLiveQuery<SessionSummaryRow>(sessionsListQuery);

  const tabItems = useMemo(() => {
    const titles = new Map(
      (sessionsQuery.data ?? []).map((session) => [
        session.id,
        session.title || "Untitled session",
      ]),
    );

    return tabs.map((tab) => ({
      tab,
      id: uniqueIdFromTab(tab),
      title:
        tab.type === "sessions"
          ? (titles.get(tab.id) ?? "Untitled session")
          : getStubTabLabel(tab.type),
    }));
  }, [sessionsQuery.data, tabs]);

  const shortcutIndexes = useMemo(() => {
    return new Map(
      tabItems.map(({ id }, index) => [
        id,
        index < 8 ? index + 1 : index === tabItems.length - 1 ? 9 : undefined,
      ]),
    );
  }, [tabItems]);

  const createMutation = useMutation({
    mutationFn: async () => {
      const now = new Date();
      const hours = now.getHours();
      const minutes = now.getMinutes();
      const period = hours < 12 ? "am" : "pm";
      const hour = hours % 12 || 12;
      const time =
        minutes === 0
          ? `${hour}${period}`
          : `${hour}:${String(minutes).padStart(2, "0")}${period}`;
      const nowIso = now.toISOString();
      const sessionId = crypto.randomUUID();

      await db.insert(sessions).values({
        id: sessionId,
        title: `Ad-hoc conversation at ${time}`,
        rawMd: "",
        folderId: "",
        eventJson: "",
        createdAt: nowIso,
        updatedAt: nowIso,
      });

      return { id: sessionId };
    },
    onSuccess: (session) => {
      openNew({ type: "sessions", id: session.id });
    },
  });

  return (
    <ShellView
      tabItems={tabItems}
      currentTab={currentTab}
      canGoBack={canGoBack}
      canGoNext={canGoNext}
      isMac={isMac}
      isChatOpen={isChatOpen}
      shortcutIndexes={shortcutIndexes}
      openNoteDialog={
        openNoteDialog ?? (
          <OpenNoteDialogContainer
            open={isSearchOpen}
            onOpenChange={setIsSearchOpen}
          />
        )
      }
      profileMenu={<ProfileMenuContainer />}
      updateBanner={<UpdateBannerContainer />}
      body={body}
      onSelect={select}
      onClose={close}
      onReorder={reorder}
      onGoBack={goBack}
      onGoNext={goNext}
      onHome={() => {
        if (!currentTab) {
          window.dispatchEvent(new CustomEvent("scroll-to-today"));
          return;
        }

        clearSelection();
      }}
      onAdHoc={() => createMutation.mutate()}
      onOpenSearch={() => setIsSearchOpen(true)}
      onToggleChat={() => setIsChatOpen((current) => !current)}
    />
  );
}
