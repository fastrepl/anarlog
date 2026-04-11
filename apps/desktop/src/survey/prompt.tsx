import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import {
  commands as analyticsCommands,
  type AnalyticsPayload,
} from "@hypr/plugin-analytics";

import {
  buildOnboardingSurveyDismissedPayload,
  buildOnboardingSurveyShownPayload,
  buildOnboardingSurveySubmittedPayload,
  type SurveyResponses,
} from "./config";
import { OnboardingSurveyDialog } from "./dialog";
import {
  finishOnboardingSurvey,
  onboardingSurveyQueryKey,
  recordOnboardingSurveyLaunch,
} from "./state";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

let hasRecordedOnboardingSurveyLaunch = false;

async function trackSurveyEvent(payload: AnalyticsPayload) {
  try {
    await analyticsCommands.event(payload);
  } catch {
    return;
  }
}

export function OnboardingSurveyPrompt() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);

  const completeSurveyMutation = useMutation({
    mutationFn: async (
      input:
        | { type: "dismiss" }
        | { type: "submit"; responses: SurveyResponses },
    ) => {
      if (input.type === "dismiss") {
        await trackSurveyEvent(buildOnboardingSurveyDismissedPayload());
      } else {
        await trackSurveyEvent(
          buildOnboardingSurveySubmittedPayload(input.responses),
        );
      }

      return finishOnboardingSurvey();
    },
    onSuccess: (state) => {
      queryClient.setQueryData(onboardingSurveyQueryKey, state);
      setOpen(false);
    },
  });

  useMountEffect(() => {
    if (hasRecordedOnboardingSurveyLaunch) {
      return;
    }

    hasRecordedOnboardingSurveyLaunch = true;

    let cancelled = false;

    void recordOnboardingSurveyLaunch()
      .then((state) => {
        queryClient.setQueryData(onboardingSurveyQueryKey, state);

        if (!cancelled && state.launchCount >= 2 && !state.done) {
          void trackSurveyEvent(buildOnboardingSurveyShownPayload());
          setOpen(true);
        }
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  });

  if (!open) {
    return null;
  }

  return (
    <OnboardingSurveyDialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !completeSurveyMutation.isPending) {
          completeSurveyMutation.mutate({ type: "dismiss" });
        }
      }}
      onSubmit={(responses) =>
        completeSurveyMutation.mutate({ type: "submit", responses })
      }
      submitting={completeSurveyMutation.isPending}
    />
  );
}
