import { Trans } from "@lingui/react/macro";
import { CheckCircle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { PRO_TRIAL_DAYS } from "@anlg/pricing";

import { StepRow } from "../shared";
import { type TrialPhase, useTrialFlow } from "./trial";

function TrialStatusDisplay({ trialPhase }: { trialPhase: TrialPhase }) {
  const trialDays = PRO_TRIAL_DAYS;

  return (
    <div {...stylex.props(styles.trialStatus)}>
      <StepRow status="done" label={<Trans>Signed in</Trans>} />

      {trialPhase === "checking" && (
        <StepRow
          status="active"
          label={<Trans>Checking trial eligibility...</Trans>}
        />
      )}

      {trialPhase === "starting" && (
        <>
          <StepRow
            status="done"
            label={<Trans>Eligible for free trial</Trans>}
          />
          <StepRow
            status="active"
            label={<Trans>Starting your trial...</Trans>}
          />
        </>
      )}

      {trialPhase === "already-paid" && (
        <StepRow status="done" label={<Trans>You have an active plan</Trans>} />
      )}

      {trialPhase === "already-trialing" && (
        <StepRow status="done" label={<Trans>You're on a Pro trial</Trans>} />
      )}

      {typeof trialPhase === "object" && trialPhase.done === "started" && (
        <>
          <StepRow
            status="done"
            label={<Trans>Eligible for free trial</Trans>}
          />
          <StepRow
            status="done"
            label={<Trans>Trial activated - {trialDays} days of Pro</Trans>}
          />
        </>
      )}

      {typeof trialPhase === "object" && trialPhase.done === "not_eligible" && (
        <StepRow
          status="done"
          label={<Trans>Continuing without trial</Trans>}
        />
      )}

      {typeof trialPhase === "object" && trialPhase.done === "error" && (
        <>
          <StepRow
            status="done"
            label={<Trans>Eligible for free trial</Trans>}
          />
          <StepRow
            status="failed"
            label={<Trans>Could not start trial</Trans>}
          />
        </>
      )}
    </div>
  );
}

export function AfterLogin({ onContinue }: { onContinue: () => void }) {
  const trialPhase = useTrialFlow(onContinue);

  if (trialPhase) {
    return <TrialStatusDisplay trialPhase={trialPhase} />;
  }

  return (
    <div {...stylex.props(styles.signedIn)}>
      <CheckCircle {...stylex.props(styles.icon)} />
      <span>
        <Trans>Signed in</Trans>
      </span>
    </div>
  );
}

const styles = stylex.create({
  icon: {
    height: "1rem",
    width: "1rem",
  },
  signedIn: {
    alignItems: "center",
    color: "rgb(5 150 105)",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
  },
  trialStatus: {
    display: "flex",
    flexDirection: "column",
    gap: "0.375rem",
  },
});
