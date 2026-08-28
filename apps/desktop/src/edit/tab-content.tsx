import { Trans } from "@lingui/react/macro";
import { MultiFileDiff } from "@pierre/diffs/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useMemo, useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

import { useStrictModeUnmount } from "./hooks";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import {
  applyProposalReview,
  declineProposalReview,
  shouldAutoDeclineProposal,
} from "~/session/proposal-review";
import {
  loadSessionProposal,
  useEnhancedNote,
  useSessionSummary,
} from "~/session/queries";
import { StandardContentWrapper } from "~/shared/main";
import type { Tab } from "~/store/zustand/tabs";

type EditTab = Extract<Tab, { type: "edit" }>;

export function TabContentEdit({ tab }: { tab: EditTab }) {
  const storeEdit = usePendingEditStore((state) =>
    state.edits.get(tab.requestId),
  );
  const { data: proposal, isLoading } = useQuery({
    queryKey: ["session-proposal", tab.requestId],
    queryFn: () => loadSessionProposal(tab.requestId),
    enabled: !storeEdit,
  });
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const edit = storeEdit
    ? {
        sessionId: storeEdit.sessionId,
        target: storeEdit.target,
        currentContent: storeEdit.currentContent,
        proposedContent: storeEdit.proposedContent,
        source: storeEdit.source ?? "chat",
      }
    : proposal && proposal.status === "pending"
      ? {
          sessionId: proposal.sessionId,
          target:
            proposal.kind === "memo_replace"
              ? ({ kind: "memo" } as const)
              : {
                  kind: "summary" as const,
                  enhancedNoteId: proposal.targetId,
                },
          currentContent: proposal.currentMarkdown,
          proposedContent: proposal.proposedMarkdown,
          source: proposal.source,
        }
      : null;

  const session = useSessionSummary(edit?.sessionId ?? "");
  const summary = useEnhancedNote(
    edit?.target.kind === "summary" ? edit.target.enhancedNoteId : "",
  );
  const sessionTitle = session?.title.trim() || null;
  const summaryTitle = summary?.title.trim() || null;
  const isMemo = edit?.target.kind === "memo";

  const declineOnUnmount = useCallback(() => {
    const still = usePendingEditStore.getState().edits.get(tab.requestId);
    if (still && shouldAutoDeclineProposal(still.source)) {
      void declineProposalReview(tab.requestId, queryClient);
    }
  }, [queryClient, tab.requestId]);
  useStrictModeUnmount(declineOnUnmount);

  const oldFile = useMemo(
    () =>
      edit
        ? {
            name: isMemo ? "memo.md" : "summary.md",
            contents: edit.currentContent || "",
          }
        : null,
    [edit, isMemo],
  );
  const newFile = useMemo(
    () =>
      edit
        ? {
            name: isMemo ? "memo.md" : "summary.md",
            contents: edit.proposedContent,
          }
        : null,
    [edit, isMemo],
  );

  const review = async (approved: boolean) => {
    setBusy(true);
    setError(null);
    try {
      if (approved) {
        await applyProposalReview(tab.requestId, queryClient);
      } else {
        await declineProposalReview(tab.requestId, queryClient);
      }
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : "Failed to update this proposal.",
      );
      setBusy(false);
    }
  };

  if (!edit && !storeEdit && isLoading) {
    return (
      <StandardContentWrapper>
        <div {...stylex.props(styles.centerMessage)}>
          <Trans>Loading edit…</Trans>
        </div>
      </StandardContentWrapper>
    );
  }

  if (!edit) {
    return (
      <StandardContentWrapper>
        <div {...stylex.props(styles.centerMessage)}>
          <Trans>This edit is no longer pending.</Trans>
        </div>
      </StandardContentWrapper>
    );
  }

  return (
    <StandardContentWrapper>
      <div {...stylex.props(styles.root)}>
        <div {...stylex.props(styles.header)}>
          <div {...stylex.props(styles.headerContent)}>
            <div {...stylex.props(styles.sessionTitle)}>
              {sessionTitle ?? <Trans>Untitled session</Trans>}
            </div>
            <div {...stylex.props(styles.summaryTitle)}>
              {isMemo ? (
                <Trans>Memo</Trans>
              ) : (
                (summaryTitle ?? <Trans>Summary</Trans>)
              )}
            </div>
          </div>
          <div {...stylex.props(styles.actions)}>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => void review(false)}
            >
              <Trans>Decline</Trans>
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={busy}
              onClick={() => void review(true)}
            >
              {isMemo ? (
                <Trans>Apply to memo</Trans>
              ) : (
                <Trans>Apply to summary</Trans>
              )}
            </Button>
          </div>
        </div>
        {error ? <div {...stylex.props(styles.error)}>{error}</div> : null}
        <div {...stylex.props(styles.diff)}>
          <MultiFileDiff
            oldFile={oldFile!}
            newFile={newFile!}
            options={{ diffStyle: "unified" }}
          />
        </div>
      </div>
    </StandardContentWrapper>
  );
}

const styles = stylex.create({
  actions: {
    display: "flex",
    flexShrink: 0,
    gap: "0.5rem",
  },
  centerMessage: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    height: "100%",
    justifyContent: "center",
  },
  diff: {
    flex: "1",
    overflow: "auto",
  },
  error: {
    backgroundColor: "#fef2f2",
    color: "#dc2626",
    fontSize: "0.8125rem",
    paddingBlock: "0.5rem",
    paddingInline: "1rem",
  },
  header: {
    alignItems: "flex-start",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  headerContent: {
    flex: "1",
    minWidth: 0,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  sessionTitle: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    fontWeight: 500,
  },
  summaryTitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
});
