import { FieldGroup, Switch, Text } from "@expo/ui";
import { useMutation, useQuery } from "@tanstack/react-query";
import * as LocalAuthentication from "expo-local-authentication";

import { authenticateForAppLock } from "@/settings/app-lock";
import { SettingsError, SettingsPage } from "@/settings/components";
import {
  setPrivacyPreference,
  usePrivacyPreferences,
} from "@/settings/privacy-store";

export default function PrivacySettings() {
  const preferences = usePrivacyPreferences();
  const security = useQuery({
    queryKey: ["device-security"],
    queryFn: () => LocalAuthentication.getEnrolledLevelAsync(),
  });
  const save = useMutation({
    mutationFn: async ({
      key,
      value,
    }: {
      key: "analytics" | "errorReports" | "appLock";
      value: boolean;
    }) => {
      if (key === "appLock" && !(await authenticateForAppLock())) return;
      await setPrivacyPreference(key, value);
    },
  });
  return (
    <SettingsPage title="Privacy">
      <FieldGroup.Section>
        <Switch
          label="Lock app"
          value={preferences.appLock}
          disabled={save.isPending || !security.data}
          onValueChange={(value) => save.mutate({ key: "appLock", value })}
        />
        <FieldGroup.SectionFooter>
          <Text>
            {security.data === LocalAuthentication.SecurityLevel.NONE
              ? "Set up a device passcode to lock Anarlog."
              : "Require Face ID, fingerprint, or your device passcode to open Anarlog. Recording continues while the app is locked."}
          </Text>
        </FieldGroup.SectionFooter>
      </FieldGroup.Section>
      <FieldGroup.Section title="Help improve Anarlog">
        <Switch
          label="Usage analytics"
          value={preferences.analytics}
          disabled={save.isPending}
          onValueChange={(value) => save.mutate({ key: "analytics", value })}
        />
        <Switch
          label="Error reports"
          value={preferences.errorReports}
          disabled={save.isPending}
          onValueChange={(value) => save.mutate({ key: "errorReports", value })}
        />
        <FieldGroup.SectionFooter>
          <Text>
            Share anonymous usage and sanitized error reports. Notes,
            transcripts, recordings, and API keys are excluded.
          </Text>
        </FieldGroup.SectionFooter>
        <SettingsError error={save.error || security.error} />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
