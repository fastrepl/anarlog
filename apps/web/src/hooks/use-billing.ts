import {
  keepPreviousData,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { jwtDecode } from "jwt-decode";
import { useCallback, useEffect, useState } from "react";

import {
  type BillingInfo,
  deriveBillingInfo,
  type SupabaseJwtPayload,
} from "@anlg/supabase";

import { getSupabaseBrowserClient } from "@/functions/supabase";

const DEFAULT_BILLING = deriveBillingInfo(null);

function deriveBillingFromAccessToken(accessToken: string) {
  return deriveBillingInfo(jwtDecode<SupabaseJwtPayload>(accessToken));
}

export function useBilling() {
  const queryClient = useQueryClient();
  const [accessToken, setAccessToken] = useState<string | null | undefined>(
    undefined,
  );

  useEffect(() => {
    const supabase = getSupabaseBrowserClient();

    void supabase.auth.getSession().then(({ data }) => {
      setAccessToken(data.session?.access_token ?? null);
    });

    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((_event, session) => {
      setAccessToken(session?.access_token ?? null);
    });

    return () => {
      subscription.unsubscribe();
    };
  }, []);

  const jwtQuery = useQuery({
    queryKey: ["billing", "jwt", accessToken ?? ""],
    queryFn: async () => {
      if (!accessToken) {
        return DEFAULT_BILLING;
      }

      return deriveBillingFromAccessToken(accessToken);
    },
    enabled: accessToken !== undefined,
    // Keep isReady stable across token refreshes so consumers gating on it
    // (e.g. the integration connect flow) do not unmount mid-flow.
    placeholderData: keepPreviousData,
    retry: false,
  });

  const billing: BillingInfo = jwtQuery.data ?? DEFAULT_BILLING;
  const isReady = accessToken !== undefined && !jwtQuery.isPending;
  const isVerified = isReady;

  const refreshBilling = useCallback(async () => {
    const supabase = getSupabaseBrowserClient();
    const { data, error } = await supabase.auth.refreshSession();

    if (error) {
      throw error;
    }

    const refreshedAccessToken = data.session?.access_token;

    if (!refreshedAccessToken) {
      throw new Error("Billing refresh returned no authenticated session");
    }

    const refreshedBilling = deriveBillingFromAccessToken(refreshedAccessToken);
    setAccessToken(refreshedAccessToken);
    await queryClient.invalidateQueries({ queryKey: ["billing", "jwt"] });

    return refreshedBilling;
  }, [queryClient]);

  return {
    ...billing,
    isReady,
    isVerified,
    refreshBilling,
  };
}
