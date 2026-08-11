import { useQuery } from "@tanstack/react-query";
import { jwtDecode } from "jwt-decode";

import { deriveBillingInfo, type SupabaseJwtPayload } from "@anlg/supabase";

import { getSupabaseBrowserClient } from "@/functions/supabase";

export const accountSessionQueryKey = ["account-session"];

export function useAccountSession() {
  return useQuery({
    queryKey: accountSessionQueryKey,
    queryFn: async () => {
      const supabase = getSupabaseBrowserClient();
      const { data } = await supabase.auth.getSession();
      const session = data.session;
      if (!session) {
        return null;
      }

      return {
        billing: deriveBillingInfo(
          jwtDecode<SupabaseJwtPayload>(session.access_token),
        ),
        createdAt: session.user.created_at ?? null,
      };
    },
  });
}
