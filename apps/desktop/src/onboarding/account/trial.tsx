import * as Sentry from "@sentry/react";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import { startTrial } from "@hypr/api-client";
import type { StartTrialReason } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { configureProSettings } from "~/shared/config/configure-pro-settings";
import * as settings from "~/store/tinybase/store/settings";

export type TrialPhase =
  | "checking"
  | "starting"
  | "already-pro"
  | "already-trialing"
  | { done: StartTrialReason | "error" };

export function useTrialFlow(onContinue: () => void) {
  const auth = useAuth();
  const billing = useBillingAccess();
  const store = settings.UI.useStore(settings.STORE_ID);
  const hasTriggeredRef = useRef(false);

  const mutation = useMutation({
    mutationFn: async () => {
      const headers = auth.getHeaders();
      if (!headers) throw new Error("no headers");
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await startTrial({
        client,
        query: { interval: "monthly" },
      });
      if (error) throw error;
      return data;
    },
    onSuccess: async (data) => {
      if (data?.started && store) {
        configureProSettings(store);
      }
      await auth.refreshSession();
      await new Promise((r) => setTimeout(r, data?.started ? 3000 : 1500));
      onContinue();
    },
    onError: async (e) => {
      Sentry.captureException(e);
      void analyticsCommands.event({
        event: "trial_flow_client_error",
        properties: { error: String(e) },
      });
      await new Promise((r) => setTimeout(r, 1500));
      onContinue();
    },
  });

  useEffect(() => {
    if (!auth?.session || !billing.isReady || hasTriggeredRef.current) return;

    if (billing.isPro && !billing.isTrialing) {
      hasTriggeredRef.current = true;
      void analyticsCommands.event({
        event: "trial_flow_skipped",
        properties: { reason: "already_pro" },
      });
      if (store) configureProSettings(store);
      setTimeout(onContinue, 1500);
      return;
    }

    if (billing.isTrialing) {
      hasTriggeredRef.current = true;
      void analyticsCommands.event({
        event: "trial_flow_skipped",
        properties: { reason: "already_trialing" },
      });
      if (store) configureProSettings(store);
      setTimeout(onContinue, 1500);
      return;
    }

    hasTriggeredRef.current = true;
    mutation.mutate();
  }, [auth, billing, store, mutation, onContinue]);

  if (!auth?.session) return null;
  if (!billing.isReady) return "checking" as const;

  if (billing.isPro && !billing.isTrialing) return "already-pro" as const;
  if (billing.isTrialing) return "already-trialing" as const;

  if (mutation.isPending) return "starting" as const;

  if (mutation.isSuccess) {
    const reason = mutation.data?.reason;
    if (reason === "started" || reason === "not_eligible") {
      return { done: reason };
    }
    return { done: "error" as const };
  }

  if (mutation.isError) {
    return { done: "error" as const };
  }

  return "checking" as const;
}
