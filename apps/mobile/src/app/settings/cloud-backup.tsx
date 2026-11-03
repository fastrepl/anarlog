import { useMutation } from "@tanstack/react-query";

import { useAuth } from "@/auth/context";
import { confirmDestructive } from "@/lib/confirm";
import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";
import { FieldGroup } from "@/settings/field-group";
import { Button, Text } from "@/settings/fields";
import { restoreDatabaseFromR2 } from "@/settings/r2-database-backup";

export default function CloudBackupSettings() {
  const auth = useAuth();
  const restore = useMutation({
    mutationFn: async () => {
      const accessToken = auth.session?.access_token;
      if (!accessToken) throw new Error("Sign in to restore your backup.");
      await restoreDatabaseFromR2(accessToken);
    },
  });
  return (
    <SettingsPage title="R2 backup">
      <FieldGroup.Section>
        <SettingsRow
          title="Automatic backup"
          value={
            !auth.session
              ? "Sign in required"
              : auth.billing.isPro
                ? "Enabled for this account"
                : "Anarlog Pro required"
          }
          description="Encrypted notes, transcripts, app data, and audio recordings are backed up to your private R2 bucket while Anarlog is open."
        />
        <Button
          label={
            restore.isPending ? "Preparing restore…" : "Restore latest backup"
          }
          disabled={!auth.session || !auth.billing.isPro || restore.isPending}
          onPress={async () => {
            if (
              await confirmDestructive(
                "The backup will replace this iPhone’s local database when Anarlog next starts. Continue?",
                "Restore backup",
              )
            )
              restore.mutate();
          }}
        />
        {restore.isSuccess && (
          <Text>
            Backup downloaded. Force close and reopen Anarlog to apply it.
          </Text>
        )}
        <Text>
          Database backups are encrypted on this device. The encryption key is
          kept in this iPhone’s Keychain, so database restore currently requires
          this same device. Audio files use Anarlog’s end-to-end encrypted
          attachment backup.
        </Text>
        <SettingsError error={restore.error} />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
