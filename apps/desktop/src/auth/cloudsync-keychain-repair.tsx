import { useLingui } from "@lingui/react/macro";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useSyncExternalStore } from "react";

import { useAuth } from "./auth-context";
import {
  applyCloudsyncPreference,
  getCloudsyncCredentialBlock,
  subscribeCloudsyncCredentialBlock,
} from "./cloudsync";
import { setCredentialBlock } from "./cloudsync-credentials";

import { repairKeychainAccess } from "~/shared/keychain";
import { SettingsAlertToast } from "~/shared/ui/settings-alert";

const TOAST_ID = "cloudsync-keychain-access";

export function CloudsyncKeychainRepairToast() {
  const { t } = useLingui();
  const auth = useAuth();
  const queryClient = useQueryClient();
  const credentialBlock = useSyncExternalStore(
    subscribeCloudsyncCredentialBlock,
    getCloudsyncCredentialBlock,
    getCloudsyncCredentialBlock,
  );
  const repairMutation = useMutation({
    mutationKey: ["repair-keychain-access", "cloudsync-toast"],
    mutationFn: async () => {
      await repairKeychainAccess();
      setCredentialBlock(null);
      const result = await applyCloudsyncPreference(auth.session);
      if (result === "account_mismatch") {
        await auth.signOut();
      }
    },
    onSettled: async () => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: ["e2ee-identity", auth.session?.user.id],
        }),
        queryClient.invalidateQueries({
          queryKey: ["cloudsync-status-indicator"],
        }),
        queryClient.invalidateQueries({
          queryKey: ["cloudsync-status-settings"],
        }),
      ]);
    },
  });
  const description =
    repairMutation.error?.message ??
    t`macOS could not access your recovery key. Repair Keychain access, then resume sync.`;
  const needsRepair =
    Boolean(auth.session) && credentialBlock === "keychain_access";

  return (
    <SettingsAlertToast
      id={TOAST_ID}
      description={needsRepair ? description : undefined}
      variant="error"
      lifecycle="condition-bound"
      action={
        repairMutation.isPending
          ? undefined
          : {
              label: t`Repair Keychain Access`,
              onClick: () => repairMutation.mutate(),
            }
      }
    />
  );
}
