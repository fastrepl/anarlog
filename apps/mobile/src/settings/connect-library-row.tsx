import { useMutation } from "@tanstack/react-query";
import { Alert } from "react-native";

import { supabase } from "@/auth/client";
import { useAuth } from "@/auth/context";
import { connectLocalLibrary, execute } from "@/db";
import { SettingsError } from "@/settings/components";
import { Button } from "@/settings/fields";
import type { MobileSyncPhase } from "@/sync/controller";
import { retryMobileSync } from "@/sync/mobile-sync";

export function ConnectLibraryRow({ phase }: { phase: MobileSyncPhase }) {
  const auth = useAuth();
  const connect = useMutation({
    mutationFn: async (accountUserId: string | undefined) => {
      if (!accountUserId) throw new Error("Sign in to connect this library");
      const [library] = await execute<{ workspace_id: string }>(
        "SELECT json_extract(value_json, '$.workspace_id') AS workspace_id FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
        [],
      );
      if (!library?.workspace_id)
        throw new Error("Could not read the local library");
      const current = await supabase?.auth.getSession();
      if (current?.data.session?.user.id !== accountUserId)
        throw new Error("The signed-in account changed. Try again.");
      await connectLocalLibrary(accountUserId, library.workspace_id);
      retryMobileSync();
    },
  });
  if (phase !== "account_mismatch") return null;
  return (
    <>
      <Button
        label={connect.isPending ? "Connecting…" : "Connect this local library"}
        disabled={connect.isPending}
        onPress={() =>
          Alert.alert(
            `Sync this local library with ${auth.session?.user.email ?? "this account"}?`,
            "Your personal notes and recordings stay on this device. Sync will connect them to this account and bring in its existing notes. Copies in your previous account remain there. Team and shared notes stay with their workspace. Reconnect integrations for this account. After connecting, this library requires an app version that supports account switching.",
            [
              { text: "Keep using locally", style: "cancel" },
              {
                text: "Connect library",
                onPress: () => connect.mutate(auth.session?.user.id),
              },
            ],
          )
        }
      />
      <SettingsError error={connect.error} />
    </>
  );
}
