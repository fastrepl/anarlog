import { FieldGroup, Text } from "@expo/ui";
import { useMutation } from "@tanstack/react-query";
import * as WebBrowser from "expo-web-browser";

import { useAuth } from "@/auth/context";
import { env } from "@/lib/env";
import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";

export default function AccountSettings() {
  const auth = useAuth();
  const refresh = useMutation({ mutationFn: auth.refreshBilling });
  const signOut = useMutation({ mutationFn: auth.signOut });
  const manage = useMutation({
    mutationFn: () =>
      WebBrowser.openBrowserAsync(
        `${env.appUrl.replace(/\/+$/, "")}/app/account`,
      ),
  });
  return (
    <SettingsPage title="Account">
      <FieldGroup.Section>
        <SettingsRow
          title="Email"
          value={auth.session?.user.email ?? "Not signed in"}
        />
        <SettingsRow
          title="Plan"
          value={
            auth.bypass
              ? "Local dev"
              : auth.billing.plan === "trial"
                ? `Pro trial · ${auth.billing.trialDaysRemaining ?? 0} days left`
                : auth.billing.plan === "pro"
                  ? "Anarlog Pro"
                  : "Free"
          }
        />
      </FieldGroup.Section>
      {!auth.bypass && (
        <FieldGroup.Section>
          <SettingsRow
            title="Manage account & subscription"
            onPress={() => manage.mutate()}
          />
          <SettingsRow
            title={refresh.isPending ? "Refreshing…" : "Refresh plan"}
            onPress={() => refresh.mutate()}
          />
          <SettingsError error={refresh.error || manage.error} />
        </FieldGroup.Section>
      )}
      {!auth.bypass && (
        <FieldGroup.Section>
          <SettingsRow
            title={signOut.isPending ? "Signing out…" : "Sign out"}
            onPress={() => signOut.mutate()}
          />
          <SettingsError error={signOut.error} />
        </FieldGroup.Section>
      )}
      {auth.bypass && (
        <FieldGroup.SectionFooter>
          <Text>This development build works locally without an account.</Text>
        </FieldGroup.SectionFooter>
      )}
    </SettingsPage>
  );
}
