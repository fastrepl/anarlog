import { FieldGroup, Text } from "@expo/ui";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  getRecordingPermissionsAsync,
  requestRecordingPermissionsAsync,
} from "expo-audio";
import { useRouter } from "expo-router";
import { Linking, Platform } from "react-native";

import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";

export default function RecordingSettings() {
  const router = useRouter();
  const permission = useQuery({
    queryKey: ["microphone-permission"],
    queryFn: getRecordingPermissionsAsync,
    refetchInterval: 2000,
  });
  const request = useMutation({
    mutationFn: async () => {
      if (permission.data?.canAskAgain && !permission.data.granted)
        await requestRecordingPermissionsAsync();
      else await Linking.openSettings();
      await permission.refetch();
    },
  });
  return (
    <SettingsPage title="Recording">
      {Platform.OS === "ios" && (
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
