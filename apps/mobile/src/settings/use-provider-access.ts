import { useQuery } from "@tanstack/react-query";

import { useAuth } from "@/auth/context";

import { readProviderConfig } from "./providers";
import type { ProviderKind } from "./providers-model";

export function useProviderAccess(kind: ProviderKind) {
  const auth = useAuth();
  const account = auth.session?.user.id ?? null;
  const config = useQuery({
    queryKey: ["provider", account, kind],
    queryFn: () => readProviderConfig(account, kind),
  });
  return (
    auth.bypass ||
    auth.billing.isPro ||
    Boolean(config.data && config.data.provider !== "anarlog")
  );
}
