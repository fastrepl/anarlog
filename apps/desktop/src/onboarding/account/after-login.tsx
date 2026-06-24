import { Trans } from "@lingui/react/macro";
import { CheckCircle2Icon } from "lucide-react";

import { StepRow } from "../shared";
import { type TrialPhase, useTrialFlow } from "./trial";

function TrialStatusDisplay({ trialPhase }: { trialPhase: TrialPhase }) {
  return (
    <div className="flex flex-col gap-1.5">
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
            label={<Trans>Trial activated - 14 days of Pro</Trans>}
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
    <div className="flex items-center gap-2 text-sm text-emerald-600">
      <CheckCircle2Icon className="size-4" />
      <span>
        <Trans>Signed in</Trans>
      </span>
    </div>
  );
}
