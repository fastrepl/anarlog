import { useRouter } from "expo-router";

import { useMicrophonePermission } from "@/audio/microphone-permission";
import { deviceHasActionButton } from "@/quick-actions/action-button-setup";
import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";
import { FieldGroup } from "@/settings/field-group";
import { Text } from "@/settings/fields";

export default function RecordingSettings() {
  const router = useRouter();
  const { permission, request } = useMicrophonePermission();
  return (
    <SettingsPage title="Recording">
      {deviceHasActionButton && (
        <FieldGroup.Section>
          <SettingsRow
            title="Action Button"
            description="Start or stop listening in one press"
            onPress={() => router.push("/action-button")}
          />
        </FieldGroup.Section>
      )}
      <FieldGroup.Section>
        <SettingsRow
          title="Microphone"
          value={
            permission.isPending
              ? "Checking…"
              : permission.data?.granted
                ? "Allowed"
                : "Allow microphone access"
          }
          onPress={() => request.mutate()}
        />
        <SettingsError error={permission.error || request.error} />
      </FieldGroup.Section>
      <FieldGroup.Section>
        <SettingsRow
          title="Recording storage"
          description="View recordings saved on this device"
          onPress={() => router.push("/settings/sync")}
        />
        <FieldGroup.SectionFooter>
          <Text>
            Recordings are saved on this device first. Keep Anarlog open to
            finish backing them up.
          </Text>
        </FieldGroup.SectionFooter>
      </FieldGroup.Section>
    </SettingsPage>
  );
}
