import { useQuery } from "@tanstack/react-query";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { canStartTrial as canStartTrialApi } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as authCommands } from "@hypr/plugin-auth";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { openUrlWithInstruction } from "@hypr/plugin-windows";
import {
  type BillingInfo,
  deriveBillingInfo,
  type SupabaseJwtPayload,
} from "@hypr/supabase";

import { TrialEndedDialog } from "../billing/trial-ended-dialog";
import { TrialStartedDialog } from "../billing/trial-started-dialog";
import { env } from "../env";
import { configurePaidSettings } from "../shared/config/configure-paid-settings";
import { buildWebAppUrl } from "../shared/utils";
import { useAuth } from "./context";

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
  };
}

type BillingContextValue = BillingInfo & {
  isReady: boolean;
  canStartTrial: { data: boolean; isPending: boolean };
  upgradeToPro: () => void;
};

export type BillingAccess = BillingContextValue;

const BillingContext = createContext<BillingContextValue | null>(null);

const TRIAL_STARTED_SEEN_PREFIX = "anarlog:trial_started_seen:";
const TRIAL_ENDED_SEEN_PREFIX = "anarlog:trial_ended_seen:";

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
  });

  const billing = deriveBillingInfo(claimsQuery.data ?? null);
  const isReady = !claimsQuery.isPending && !claimsQuery.isError;

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

  const upgradeToPro = useCallback(async () => {
    const url = await buildWebAppUrl("/app/checkout", { period: "monthly" });
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
    hasTrial,
    billing.isPaid,
    isReady,
    canTrialQuery.data?.reason,
    canTrialQuery.isPending,
    trialEligibilityRefreshedUserId,
    auth.refreshSession,
  ]);

  const value = useMemo<BillingContextValue>(
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
      />
      <TrialEndedDialog
        open={trialEndedOpen}
        onOpenChange={setTrialEndedOpen}
        onUpgrade={upgradeToPro}
      />
    </BillingContext.Provider>
  );
}

export function useBillingAccess() {
  const context = useContext(BillingContext);

  if (!context) {
    throw new Error("useBillingAccess must be used within BillingProvider");
  }

  return context;
}
