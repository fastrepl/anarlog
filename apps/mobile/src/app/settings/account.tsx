import { useMutation, useQuery } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import * as WebBrowser from "expo-web-browser";

import { accountPlanLabel, fetchWorkspacePlan } from "@/auth/account-plan";
import { supabase } from "@/auth/client";
import { useAuth } from "@/auth/context";
import { useTrial } from "@/auth/use-trial";
import { env } from "@/lib/env";
import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";
import { FieldGroup } from "@/settings/field-group";
import { Text } from "@/settings/fields";

export default function AccountSettings() {
  const auth = useAuth();
  const trial = useTrial();
  const router = useRouter();
  const accountId = auth.session?.user.id;
  const workspacePlan = useQuery({
    queryKey: ["mobile-account-plan", accountId],
    enabled: Boolean(accountId) && !auth.bypass,
    retry: 1,
    queryFn: async ({ signal }) => {
      const current = await supabase!.auth.getSession();
      const session = current.data.session;
      if (current.error || !session || session.user.id !== accountId)
        throw new Error("Sign in again to refresh your plan.");
      return fetchWorkspacePlan({
        client: supabase!,
        accessToken: session.access_token,
        signal,
      });
    },
  });
  const refresh = useMutation({
    mutationFn: async () => {
      await auth.refreshBilling();
      await workspacePlan.refetch({ throwOnError: true });
    },
  });
  const signOut = useMutation({ mutationFn: auth.signOut });
  const manage = useMutation({
    mutationFn: () =>
      WebBrowser.openBrowserAsync(
        `${env.appUrl.replace(/\/+$/, "")}/app/account`,
      ),
  });
  const connectedAccounts = useMutation({
    mutationFn: () => {
      if (!accountId)
        throw new Error("Sign in to manage your sign-in methods.");
      const url = new URL(`${env.appUrl.replace(/\/+$/, "")}/app/account`);
      url.searchParams.set("account_user_id", accountId);
      url.searchParams.set("section", "connected-accounts");
      url.hash = "connected-accounts";
      return WebBrowser.openBrowserAsync(url.toString());
    },
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
              : workspacePlan.isPending
                ? "Checking…"
                : workspacePlan.data === undefined
                  ? "Could not verify plan"
                  : accountPlanLabel(auth.billing, workspacePlan.data)
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
                ? "Your Pro trial includes cloud sync and Anarlog models. After it ends, you can keep using your notes, recording, and your own API keys."
                : auth.billing.isPro
                  ? "Cloud sync and Anarlog models are included in your subscription."
                  : trial.isFetching
                    ? "Checking your free Pro trial. You can start taking notes and recording now."
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
            title={
              connectedAccounts.isPending
                ? "Opening connected accounts…"
                : "Connected accounts"
            }
            onPress={() => {
              if (!connectedAccounts.isPending) connectedAccounts.mutate();
            }}
          />
          <SettingsRow
            title={refresh.isPending ? "Refreshing…" : "Refresh plan"}
            onPress={() => refresh.mutate()}
          />
          <SettingsError
            error={
              refresh.error ||
              manage.error ||
              connectedAccounts.error ||
              workspacePlan.error
            }
          />
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
