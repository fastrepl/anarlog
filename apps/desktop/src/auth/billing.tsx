import { useQuery } from "@tanstack/react-query";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
} from "react";

import { canStartTrial as canStartTrialApi } from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as authCommands } from "@hypr/plugin-auth";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import {
  type BillingInfo,
  deriveBillingInfo,
  type SupabaseJwtPayload,
} from "@hypr/supabase";

import { env } from "../env";
import { buildWebAppUrl } from "../shared/utils";
import { useAuth } from "./context";

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

export function BillingProvider({ children }: { children: ReactNode }) {
  const auth = useAuth();

  const claimsQuery = useQuery({
    queryKey: ["tokenInfo", auth?.session?.access_token ?? ""],
    queryFn: () => getClaimsFromToken(auth!.session!.access_token),
    enabled: !!auth?.session?.access_token,
  });

  const billing = deriveBillingInfo(claimsQuery.data ?? null);
  const isReady = !claimsQuery.isPending;

  const canTrialQuery = useQuery({
    enabled: !!auth?.session && !billing.isPro,
    queryKey: [auth?.session?.user.id ?? "", "canStartTrial"],
    queryFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        return false;
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await canStartTrialApi({ client });
      if (error) {
        return false;
      }
      return data?.canStartTrial ?? false;
    },
  });

  const canStartTrial = useMemo(
    () => ({
      data: billing.isPro ? false : (canTrialQuery.data ?? false),
      isPending: canTrialQuery.isPending,
    }),
    [billing.isPro, canTrialQuery.data, canTrialQuery.isPending],
  );

  const upgradeToPro = useCallback(async () => {
    const url = await buildWebAppUrl("/app/checkout", { period: "monthly" });
    void openerCommands.openUrl(url, null);
  }, []);

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
    <BillingContext.Provider value={value}>{children}</BillingContext.Provider>
  );
}

export function useBillingAccess() {
  const context = useContext(BillingContext);

  if (!context) {
    throw new Error("useBillingAccess must be used within BillingProvider");
  }

  return context;
}
