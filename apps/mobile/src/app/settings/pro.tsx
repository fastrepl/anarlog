import { Redirect, useRouter } from "expo-router";

import { requiresPro } from "@/auth/companion";
import { useAuth } from "@/auth/context";
import { ProScreen } from "@/auth/screens";

export default function ProSettings() {
  const auth = useAuth();
  const router = useRouter();
  if (requiresPro) return <Redirect href="/settings/account" />;
  return (
    <ProScreen
      billing={auth.billing}
      email={auth.session?.user.email ?? ""}
      onRefreshBilling={auth.refreshBilling}
      onClose={() => router.back()}
    />
  );
}
