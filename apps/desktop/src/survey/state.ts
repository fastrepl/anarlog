import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands, type OnboardingSurveyState } from "~/types/tauri.gen";

export const onboardingSurveyQueryKey = ["onboarding-survey"] as const;

export const defaultOnboardingSurveyState: OnboardingSurveyState = {
  launchCount: 0,
  done: false,
};

function unwrapSurveyState(
  result: Awaited<ReturnType<typeof commands.getOnboardingSurveyState>>,
) {
  if (result.status === "ok") {
    return result.data;
  }

  throw new Error(result.error);
}

export async function getOnboardingSurveyState() {
  return unwrapSurveyState(await commands.getOnboardingSurveyState());
}

export async function recordOnboardingSurveyLaunch() {
  const result = await commands.recordOnboardingSurveyLaunch();

  if (result.status === "ok") {
    return result.data;
  }

  throw new Error(result.error);
}

export async function finishOnboardingSurvey() {
  const result = await commands.finishOnboardingSurvey();

  if (result.status === "ok") {
    return result.data;
  }

  throw new Error(result.error);
}

export async function resetOnboardingSurvey() {
  const result = await commands.resetOnboardingSurvey();

  if (result.status === "ok") {
    return result.data;
  }

  throw new Error(result.error);
}

export function useOnboardingSurveyState() {
  return useQuery({
    queryKey: onboardingSurveyQueryKey,
    queryFn: getOnboardingSurveyState,
    initialData: defaultOnboardingSurveyState,
  });
}

export function useResetOnboardingSurvey() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: resetOnboardingSurvey,
    onSuccess: (state) => {
      queryClient.setQueryData(onboardingSurveyQueryKey, state);
    },
  });
}
