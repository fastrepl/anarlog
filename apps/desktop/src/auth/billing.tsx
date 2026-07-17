import { useQuery } from "@tanstack/react-query";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { canStartTrial as canStartTrialApi } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as authCommands } from "@hypr/plugin-auth";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { openUrlWithInstruction } from "@hypr/plugin-windows";
import { deriveBillingInfo, type SupabaseJwtPayload } from "@hypr/supabase";

import { TrialEndedDialog } from "../billing/trial-ended-dialog";
import { TrialPaymentReminderDialog } from "../billing/trial-payment-reminder-dialog";
import { TrialStartedDialog } from "../billing/trial-started-dialog";
import { env } from "../env";
import { configurePaidSettings } from "../shared/config/configure-paid-settings";
import { buildWebAppUrl } from "../shared/utils";
import { useAuth } from "./auth-context";
import { type BillingAccess, BillingContext } from "./billing-context";

import { setSettingValues } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

async function getClaimsFromToken(
  accessToken: string,
): Promise<SupabaseJwtPayload | null> {
  const result = await authCommands.decodeClaims(accessToken);
  if (result.status === "error") {
    return null;
  }
  return {
    sub: result.data.sub,
    email: result.data.email ?? undefined,
    entitlements: result.data.entitlements,
    subscription_status: result.data.subscription_status,
    trial_end: result.data.trial_end,
    has_payment_method: result.data.has_payment_method,
  };
}

const TRIAL_STARTED_SEEN_PREFIX = "anarlog:trial_started_seen:";
const TRIAL_ENDED_SEEN_PREFIX = "anarlog:trial_ended_seen:";
const TRIAL_PAYMENT_REMINDER_SEEN_PREFIX =
  "anarlog:trial_payment_reminder_seen:";

function readSeen(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

function markSeen(key: string): void {
  try {
    localStorage.setItem(key, "1");
  } catch {
    // ignore — modal will just show again next session
  }
}

export function BillingProvider({ children }: { children: ReactNode }) {
  const auth = useAuth();
  const currentLlmProvider = useConfigValue("current_llm_provider");

  const claimsQuery = useQuery({
    queryKey: ["tokenInfo", auth?.session?.access_token ?? ""],
    queryFn: () => getClaimsFromToken(auth!.session!.access_token),
    enabled: !!auth?.session?.access_token,
    placeholderData: (previous) =>
      previous?.sub === auth?.session?.user.id ? previous : undefined,
  });

  const billing = deriveBillingInfo(claimsQuery.data ?? null);
  const isReady = !claimsQuery.isPending && !claimsQuery.isError;

  // eslint-disable-next-line @tanstack/query/exhaustive-deps -- Auth supplies request headers; the user ID is the eligibility identity.
  const canTrialQuery = useQuery({
    enabled: !!auth?.session && !billing.isPaid,
    queryKey: [auth?.session?.user.id ?? "", "canStartTrial"],
    queryFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        return { canStartTrial: false, reason: "error" as const };
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await canStartTrialApi({ client });
      if (error) {
        return { canStartTrial: false, reason: "error" as const };
      }
      return {
        canStartTrial: data?.canStartTrial ?? false,
        reason: data?.reason ?? null,
      };
    },
  });

  const canStartTrial = useMemo(
    () => ({
      data: billing.isPaid
        ? false
        : (canTrialQuery.data?.canStartTrial ?? false),
      isPending: canTrialQuery.isPending,
    }),
    [
      billing.isPaid,
      canTrialQuery.data?.canStartTrial,
      canTrialQuery.isPending,
    ],
  );

  const openUpgrade = useCallback(
    async (source: "feature_gate" | "trial_ended") => {
      void analyticsCommands.event({
        event: "upgrade_clicked",
        plan: "pro",
        period: "monthly",
        source,
      });

      const url = await buildWebAppUrl("/app/checkout", {
        period: "monthly",
        source,
      });
      await openUrlWithInstruction(url, "billing", (u) =>
        openerCommands.openUrl(u, null),
      );
    },
    [],
  );

  const upgradeToPro = useCallback(() => {
    void openUpgrade("feature_gate");
  }, [openUpgrade]);

  const openBillingPortal = useCallback(async () => {
    const url = await buildWebAppUrl("/app/portal");
    await openUrlWithInstruction(url, "billing", (u) =>
      openerCommands.openUrl(u, null),
    );
  }, []);

  useEffect(() => {
    if (!auth?.session?.user.id || !isReady || billing.isPaid) {
      return;
    }

    if (currentLlmProvider !== "hyprnote") {
      return;
    }

    void setSettingValues({
      current_llm_provider: "",
      current_llm_model: "",
    });
  }, [auth?.session?.user.id, billing.isPaid, currentLlmProvider, isReady]);

  const prevIsPaidRef = useRef(billing.isPaid);
  useEffect(() => {
    const wasPaid = prevIsPaidRef.current;
    prevIsPaidRef.current = billing.isPaid;

    if (!wasPaid && billing.isPaid && isReady) {
      void configurePaidSettings();
    }
  }, [billing.isPaid, isReady]);

  const [trialStartedOpen, setTrialStartedOpen] = useState(false);
  const [trialPaymentReminderOpen, setTrialPaymentReminderOpen] =
    useState(false);
  const [trialPaymentReminderThreshold, setTrialPaymentReminderThreshold] =
    useState<3 | 7 | null>(null);
  const [trialEndedOpen, setTrialEndedOpen] = useState(false);
  const [trialEligibilityRefreshedUserId, setTrialEligibilityRefreshedUserId] =
    useState<string | null>(null);
  const trialEligibilityRefreshPendingRef = useRef<string | null>(null);
  const hasTrial = billing.trialEnd !== null;

  useEffect(() => {
    const userId = auth?.session?.user.id;
    if (!userId || !isReady) {
      return;
    }

    if (billing.isTrialing) {
      const key = TRIAL_STARTED_SEEN_PREFIX + userId;
      if (!readSeen(key)) {
        setTrialStartedOpen(true);
        markSeen(key);
        return;
      }

      const daysRemaining = billing.trialDaysRemaining;
      const reminderThreshold =
        daysRemaining != null && daysRemaining <= 3
          ? 3
          : daysRemaining != null && daysRemaining <= 7
            ? 7
            : null;

      if (reminderThreshold && !claimsQuery.data?.has_payment_method) {
        const reminderKey = `${TRIAL_PAYMENT_REMINDER_SEEN_PREFIX}${userId}:${reminderThreshold}`;
        if (!readSeen(reminderKey)) {
          setTrialPaymentReminderThreshold(reminderThreshold);
          setTrialPaymentReminderOpen(true);
          markSeen(reminderKey);
          void analyticsCommands.event({
            event: "trial_payment_reminder_shown",
            days_remaining: daysRemaining,
            reminder_threshold: reminderThreshold,
          });
        }
      }
      return;
    }

    const isTrialIneligible =
      !canTrialQuery.isPending && canTrialQuery.data?.reason === "not_eligible";

    if (
      isTrialIneligible &&
      !hasTrial &&
      !billing.isPaid &&
      trialEligibilityRefreshedUserId !== userId
    ) {
      if (trialEligibilityRefreshPendingRef.current !== userId) {
        trialEligibilityRefreshPendingRef.current = userId;
        void auth
          .refreshSession()
          .catch(() => null)
          .finally(() => {
            setTrialEligibilityRefreshedUserId(userId);
            trialEligibilityRefreshPendingRef.current = null;
          });
      }
      return;
    }

    const hasRecentTrial =
      hasTrial ||
      (isTrialIneligible && trialEligibilityRefreshedUserId === userId);

    if (hasRecentTrial && !billing.isPaid) {
      const key = TRIAL_ENDED_SEEN_PREFIX + userId;
      if (!readSeen(key)) {
        setTrialEndedOpen(true);
        markSeen(key);
      }
    }
  }, [
    auth?.session?.user.id,
    billing.isTrialing,
    billing.trialDaysRemaining,
    claimsQuery.data?.has_payment_method,
    hasTrial,
    billing.isPaid,
    isReady,
    canTrialQuery.data?.reason,
    canTrialQuery.isPending,
    trialEligibilityRefreshedUserId,
    auth.refreshSession,
  ]);

  const value = useMemo<BillingAccess>(
    () => ({
      ...billing,
      isReady,
      canStartTrial,
      upgradeToPro,
    }),
    [billing, isReady, canStartTrial, upgradeToPro],
  );

  return (
    <BillingContext.Provider value={value}>
      {children}
      <TrialStartedDialog
        open={trialStartedOpen}
        onOpenChange={setTrialStartedOpen}
        trialDaysRemaining={billing.trialDaysRemaining}
        hasPaymentMethod={claimsQuery.data?.has_payment_method === true}
      />
      <TrialPaymentReminderDialog
        open={trialPaymentReminderOpen}
        onOpenChange={setTrialPaymentReminderOpen}
        daysRemaining={billing.trialDaysRemaining ?? 0}
        onAddPaymentMethod={() => {
          void analyticsCommands.event({
            event: "trial_payment_method_clicked",
            days_remaining: billing.trialDaysRemaining,
            reminder_threshold: trialPaymentReminderThreshold,
          });
          void openBillingPortal();
        }}
      />
      <TrialEndedDialog
        open={trialEndedOpen}
        onOpenChange={setTrialEndedOpen}
        onUpgrade={() => void openUpgrade("trial_ended")}
      />
    </BillingContext.Provider>
  );
}
