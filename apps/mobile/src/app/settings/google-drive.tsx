import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Platform } from "react-native";

import { confirmDestructive } from "@/lib/confirm";
import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";
import { FieldGroup } from "@/settings/field-group";
import { Button, Text } from "@/settings/fields";
import {
  backupToGoogleDrive,
  connectGoogleDrive,
  disconnectGoogleDrive,
  getGoogleDriveAccount,
  restoreFromGoogleDrive,
} from "@/settings/google-drive-backup";

export default function GoogleDriveBackupSettings() {
  const queryClient = useQueryClient();
  const account = useQuery({
    queryKey: ["google-drive-backup-account"],
    queryFn: getGoogleDriveAccount,
  });
  const connect = useMutation({
    mutationFn: connectGoogleDrive,
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: ["google-drive-backup-account"],
      }),
  });
  const disconnect = useMutation({
    mutationFn: disconnectGoogleDrive,
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: ["google-drive-backup-account"],
      }),
  });
  const backup = useMutation({ mutationFn: backupToGoogleDrive });
  const restore = useMutation({ mutationFn: restoreFromGoogleDrive });
  const connected = Boolean(account.data);
  return (
    <SettingsPage title="Google Drive backup">
      <FieldGroup.Section>
        <SettingsRow
          title={
            connected
              ? "Connected Google account"
              : "Google Drive not connected"
          }
          value={account.data ?? undefined}
        />
        {connected ? (
          <>
            <Button
              label={backup.isPending ? "Creating backup…" : "Back up now"}
              disabled={backup.isPending || restore.isPending}
              onPress={() => backup.mutate()}
            />
            <Button
              label={
                restore.isPending
                  ? "Preparing restore…"
                  : "Restore latest backup"
              }
              disabled={backup.isPending || restore.isPending}
              onPress={async () => {
                if (
                  await confirmDestructive(
                    "Restore will replace the data on this iPhone when Anarlog next starts. Continue?",
                    "Restore backup",
                  )
                )
                  restore.mutate();
              }}
            />
            <SettingsRow
              title={
                disconnect.isPending
                  ? "Disconnecting…"
                  : "Disconnect Google Drive"
              }
              onPress={() => disconnect.mutate()}
            />
            {restore.isSuccess && (
              <Text>
                Backup downloaded. Force close and reopen Anarlog to apply it.
              </Text>
            )}
            {backup.isSuccess && <Text>Backup saved to Google Drive.</Text>}
          </>
        ) : (
          <Button
            label={connect.isPending ? "Connecting…" : "Connect Google Drive"}
            disabled={
              Platform.OS !== "ios" || connect.isPending || !account.isFetched
            }
            onPress={() => connect.mutate()}
          />
        )}
        {Platform.OS !== "ios" && (
          <Text>
            Google Drive backup is currently available on iPhone only.
          </Text>
        )}
        <Text>
          Backups are encrypted and include notes, transcripts, and app data.
          Recordings are not included; the encryption key stays in this iPhone’s
          Keychain, so this backup can only be restored on this device.
        </Text>
        <SettingsError
          error={
            account.error ||
            connect.error ||
            disconnect.error ||
            backup.error ||
            restore.error
          }
        />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
