import { Trans } from "@lingui/react/macro";

import { Button } from "@anlg/ui/components/ui/button";

import {
  openProposalReview,
  proposalKindLabel,
} from "~/session/proposal-review";
import { usePendingSessionProposals } from "~/session/queries";

export function PendingProposalsBanner({ sessionId }: { sessionId: string }) {
  const proposals = usePendingSessionProposals(sessionId);
  if (proposals.length === 0) {
    return null;
  }

  return (
    <div className="shrink-0 px-1 pt-1 pb-2">
      <div className="border-border/70 bg-card/80 flex items-center justify-between gap-3 rounded-[22px] border px-3 py-2">
        <p className="text-foreground text-[13px] font-medium">
          {proposals.length === 1 ? (
            <Trans>1 pending edit</Trans>
          ) : (
            <Trans>{proposals.length} pending edits</Trans>
          )}
        </p>
        <div className="flex flex-wrap justify-end gap-2">
          {proposals.map((proposal) => (
            <Button
              key={proposal.id}
              type="button"
              size="sm"
              variant="outline"
              onClick={() => openProposalReview(proposal.id)}
            >
              {proposalKindLabel(proposal.kind) === "memo" ? (
                <Trans>Review memo</Trans>
              ) : (
                <Trans>Review summary</Trans>
              )}
            </Button>
          ))}
        </div>
      </div>
    </div>
  );
}
