import { FieldGroup, Text } from "@expo/ui";
import { useMutation } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import * as WebBrowser from "expo-web-browser";

import { useAuth } from "@/auth/context";
import { useTrial } from "@/auth/use-trial";
import { env } from "@/lib/env";
import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";

export default function AccountSettings() {
  const auth = useAuth();
  const trial = useTrial();
  const router = useRouter();
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
          {(!auth.billing.isPro || auth.billing.isTrialing) && (
            <SettingsRow
              title="Explore Anarlog Pro"
              onPress={() => router.push("/settings/pro")}
            />
          )}
          <FieldGroup.SectionFooter>
            <Text>
              {auth.billing.isTrialing
                ? "Your three-week Pro trial includes cloud sync and Anarlog models. After it ends, you can keep using your notes, recording, and your own API keys."
                : auth.billing.isPro
                  ? "Cloud sync and Anarlog models are included in your subscription."
                  : trial.isFetching
                    ? "Checking your free three-week Pro trial. You can start taking notes and recording now."
                    : "Notes and recording are free. Cloud sync and Anarlog models require an active Pro trial or subscription. You can also use your own API keys."}
            </Text>
          </FieldGroup.SectionFooter>
          {!auth.billing.isPro && trial.error && (
            <SettingsRow
              title="Retry trial activation"
              onPress={() => void trial.refetch()}
            />
          )}
          <SettingsError error={!auth.billing.isPro ? trial.error : null} />
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
