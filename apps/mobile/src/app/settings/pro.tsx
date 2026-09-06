import { useRouter } from "expo-router";

import { useAuth } from "@/auth/context";
import { ProScreen } from "@/auth/screens";

export default function ProSettings() {
  const auth = useAuth();
  const router = useRouter();
  return (
    <ProScreen
      billing={auth.billing}
      email={auth.session?.user.email ?? ""}
      onRefreshBilling={auth.refreshBilling}
      onClose={() => router.back()}
    />
  );
}
