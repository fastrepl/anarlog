import type { QueryClient } from "@tanstack/react-query";

import { usePendingEditStore } from "~/chat/tools/pending-edit-store";
import {
  applySessionProposal,
  declineSessionProposal,
} from "~/session/queries";
import type { SessionProposalRecord } from "~/session/queries/proposals";
import { useTabs } from "~/store/zustand/tabs";

export function shouldAutoDeclineProposal(source?: string): boolean {
  return source !== "cli" && source !== "mcp";
}

export function closeProposalReviewTab(requestId: string): void {
  const tabs = useTabs.getState();
  const reviewTab = tabs.tabs.find(
    (tab) => tab.type === "edit" && tab.requestId === requestId,
  );
  if (reviewTab) {
    tabs.close(reviewTab);
  }
}

export function openProposalReview(requestId: string): void {
  useTabs.getState().openNew({ type: "edit", requestId });
}

export async function applyProposalReview(
  requestId: string,
  queryClient?: QueryClient,
): Promise<void> {
  await applySessionProposal(requestId);
  usePendingEditStore.getState().resolveEdit(requestId, true);
  closeProposalReviewTab(requestId);
  await invalidateSessionProposals(queryClient);
}

export async function declineProposalReview(
  requestId: string,
  queryClient?: QueryClient,
): Promise<void> {
  await declineSessionProposal(requestId);
  usePendingEditStore.getState().resolveEdit(requestId, false);
  closeProposalReviewTab(requestId);
  await invalidateSessionProposals(queryClient);
}

export function proposalKindLabel(
  kind: SessionProposalRecord["kind"],
): "memo" | "summary" {
  return kind === "memo_replace" ? "memo" : "summary";
}

async function invalidateSessionProposals(
  queryClient: QueryClient | undefined,
): Promise<void> {
  if (!queryClient) {
    return;
  }
  await queryClient.invalidateQueries({ queryKey: ["session-proposals"] });
}
