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

      return deriveBillingInfo(jwtDecode<SupabaseJwtPayload>(accessToken));
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
    const { data } = await supabase.auth.refreshSession();
    setAccessToken(data.session?.access_token ?? null);
    await queryClient.invalidateQueries({ queryKey: ["billing"] });
  }, [queryClient]);

  return {
    ...billing,
    isReady,
    isVerified,
    refreshBilling,
  };
}
