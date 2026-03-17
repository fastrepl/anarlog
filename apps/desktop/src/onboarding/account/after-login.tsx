import { CheckCircle2Icon } from "lucide-react";

import { StepRow } from "../shared";
import { type TrialPhase, useTrialFlow } from "./trial";

function TrialStatusDisplay({ trialPhase }: { trialPhase: TrialPhase }) {
  return (
    <div className="flex flex-col gap-1.5">
      <StepRow status="done" label="Signed in" />

      {trialPhase === "checking" && (
        <StepRow status="active" label="Checking trial eligibility…" />
      )}

      {trialPhase === "starting" && (
        <>
          <StepRow status="done" label="Eligible for free trial" />
          <StepRow status="active" label="Starting your trial…" />
        </>
      )}

      {trialPhase === "already-pro" && (
        <StepRow status="done" label="You have an active Pro plan" />
      )}

      {trialPhase === "already-trialing" && (
        <StepRow status="done" label="You're on a Pro trial" />
      )}

      {typeof trialPhase === "object" && trialPhase.done === "started" && (
        <>
          <StepRow status="done" label="Eligible for free trial" />
          <StepRow status="done" label="Trial activated — 14 days of Pro" />
        </>
      )}

      {typeof trialPhase === "object" && trialPhase.done === "not_eligible" && (
        <StepRow status="done" label="Continuing without trial" />
      )}

      {typeof trialPhase === "object" && trialPhase.done === "error" && (
        <>
          <StepRow status="done" label="Eligible for free trial" />
          <StepRow status="failed" label="Could not start trial" />
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
      <span>Signed in</span>
    </div>
  );
}
