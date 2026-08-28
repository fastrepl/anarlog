import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";
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
    <div {...stylex.props(styles.container)}>
      <div {...stylex.props(styles.banner)}>
        <p {...stylex.props(styles.label)}>
          {proposals.length === 1 ? (
            <Trans>1 pending edit</Trans>
          ) : (
            <Trans>{proposals.length} pending edits</Trans>
          )}
        </p>
        <div {...stylex.props(styles.actions)}>
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

const styles = stylex.create({
  actions: {
    display: "flex",
    flexWrap: "wrap",
    gap: "0.5rem",
    justifyContent: "flex-end",
  },
  banner: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.card} 80%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 70%, transparent)`,
    borderRadius: "22px",
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  container: {
    flexShrink: 0,
    paddingBottom: "0.5rem",
    paddingInline: "0.25rem",
    paddingTop: "0.25rem",
  },
  label: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    fontWeight: 500,
  },
});
