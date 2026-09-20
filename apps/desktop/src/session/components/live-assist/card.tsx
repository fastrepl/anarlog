import { useLingui } from "@lingui/react/macro";
import { motion } from "motion/react";

import { md2json } from "@anlg/editor/markdown";
import {
  ChatCircle,
  CircleNotch,
  ListChecks,
  NotePencil,
  Plus,
  Sparkle,
  WarningCircle,
  X,
} from "@anlg/ui/components/icons";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@anlg/utils";

import { getStoredNoteMarkdown } from "~/session/components/note-input/header-shared";
import {
  appendLiveAssistMarkdown,
  formatLiveAssistCardMarkdown,
  type LiveAssistKind,
} from "~/session/insights/live-assist";
import { updateSession, useSession } from "~/session/queries";
import {
  useLiveAssistStore,
  type LiveAssistCard as LiveAssistCardModel,
} from "~/store/zustand/live-assist";

const KIND_ICON: Record<LiveAssistKind, typeof Sparkle> = {
  catch_up: Sparkle,
  action_items: ListChecks,
  follow_up: ChatCircle,
  summarize_so_far: NotePencil,
};

export function useLiveAssistKindLabel(kind: LiveAssistKind): string {
  const { t } = useLingui();
  switch (kind) {
    case "catch_up":
      return t`Catch Up`;
    case "action_items":
      return t`Action Items`;
    case "follow_up":
      return t`Follow Up`;
    case "summarize_so_far":
      return t`Summary So Far`;
  }
}

export function LiveAssistCard({
  sessionId,
  card,
}: {
  sessionId: string;
  card: LiveAssistCardModel;
}) {
  const { t } = useLingui();
  const kindLabel = useLiveAssistKindLabel(card.kind);
  const Icon = KIND_ICON[card.kind];
  const session = useSession(sessionId);
  const removeCard = useLiveAssistStore((state) => state.removeCard);

  const handleDismiss = () => removeCard(sessionId, card.id);

  const handleInsert = () => {
    const items = card.items;
    if (!items || items.length === 0) {
      return;
    }

    const existingMarkdown = getStoredNoteMarkdown(session?.raw_md);
    const nextMarkdown = appendLiveAssistMarkdown(
      existingMarkdown,
      formatLiveAssistCardMarkdown(kindLabel, items),
    );

    updateSession(sessionId, {
      raw_md: JSON.stringify(md2json(nextMarkdown)),
    })
      .then(() => removeCard(sessionId, card.id))
      .catch((error: unknown) => {
        console.error("Failed to insert Live Assist suggestion", error);
        sonnerToast.error(t`Could not insert into notes. Try again.`);
      });
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.15 }}
      className="border-border bg-card flex flex-col gap-2 rounded-lg border p-3"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="text-muted-foreground flex min-w-0 items-center gap-1.5 text-xs font-medium">
          <Icon aria-hidden className="size-3.5 shrink-0" />
          <span className="truncate">{kindLabel}</span>
        </div>
        <span className="text-muted-foreground shrink-0 text-xs">
          {formatDistanceToNow(new Date(card.createdAtMs), {
            addSuffix: true,
          })}
        </span>
      </div>

      {card.status === "generating" && (
        <div className="text-muted-foreground flex items-center gap-2 text-sm">
          <CircleNotch aria-hidden className="size-4 animate-spin" />
          <span>{t`Generating…`}</span>
        </div>
      )}

      {card.status === "error" && (
        <div className="text-destructive flex items-center gap-2 text-sm">
          <WarningCircle aria-hidden className="size-4 shrink-0" />
          <span>{t`Could not generate this suggestion.`}</span>
        </div>
      )}

      {card.status === "ready" && card.items && (
        <ul className="flex flex-col gap-1 text-sm">
          {card.items.map((item) => (
            <li key={item} className="flex gap-2">
              <span aria-hidden className="text-muted-foreground">
                •
              </span>
              <span className="min-w-0">{item}</span>
            </li>
          ))}
        </ul>
      )}

      {card.status !== "generating" && (
        <div className="flex items-center justify-end gap-1">
          {card.status === "ready" && (
            <button
              type="button"
              onClick={handleInsert}
              className={cn([
                "text-muted-foreground hover:bg-accent hover:text-foreground",
                "flex h-7 items-center gap-1 rounded-md px-2 text-xs font-medium transition-colors",
              ])}
            >
              <Plus aria-hidden className="size-3.5" />
              {t`Insert into notes`}
            </button>
          )}
          <button
            type="button"
            onClick={handleDismiss}
            aria-label={t`Dismiss`}
            className={cn([
              "text-muted-foreground hover:bg-accent hover:text-foreground",
              "flex size-7 items-center justify-center rounded-md transition-colors",
            ])}
          >
            <X aria-hidden className="size-3.5" />
          </button>
        </div>
      )}
    </motion.div>
  );
}
