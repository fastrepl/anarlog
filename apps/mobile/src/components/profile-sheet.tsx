import { Host, Switch } from "@expo/ui";
import { useSyncExternalStore } from "react";
import {
  KeyboardAvoidingView,
  Platform,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from "react-native";

import { useAuth } from "@/auth/context";
import { Button } from "@/components/ui/button";
import { UserAvatar } from "@/components/user-avatar";
import {
  Colors,
  CornerCurve,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";
import {
  setSidebarItemPreference,
  useSidebarItemPreferences,
} from "@/data/sidebar-preferences";
import { captureOperationalError } from "@/lib/error-reporting";
import {
  getMobileSyncSnapshot,
  retryMobileSync,
  subscribeMobileSync,
  syncMobileNow,
} from "@/sync/mobile-sync";
import { syncStatusPresentation } from "@/sync/status-presentation";

export function SettingsContent() {
  const auth = useAuth();
  const sync = useSyncExternalStore(
    subscribeMobileSync,
    getMobileSyncSnapshot,
    getMobileSyncSnapshot,
  );
  const sidebarPreferences = useSidebarItemPreferences();

  const planLabel = auth.bypass
    ? "Local dev"
    : auth.billing.plan === "trial"
      ? `Trial · ${auth.billing.trialDaysRemaining ?? 0}d left`
      : auth.billing.plan === "pro"
        ? "Pro"
        : "Free";
  const presentation = syncStatusPresentation(sync);

  const updateSidebarPreference = (
    key: "sidebar_show_folder" | "sidebar_show_tags",
    value: boolean,
  ) => {
    void setSidebarItemPreference(key, value).catch((error) => {
      captureOperationalError(error, {
        operation: "sidebar_preference_update",
        tags: { key },
      });
    });
  };

  const renderStatusActions = () => {
    if (auth.bypass || sync.phase === "inactive") return null;
    if (sync.phase === "ready") {
      return (
        <View style={styles.actions}>
          <Button
            label="Sync now"
            loading={sync.syncingNow}
            onPress={() => void syncMobileNow()}
            size="small"
            variant="outline"
          />
        </View>
      );
    }
    if (sync.phase === "not_entitled") {
      return (
        <View style={styles.actions}>
          <Button
            label="Refresh plan"
            onPress={() => void auth.refreshBilling()}
            size="small"
            variant="outline"
          />
        </View>
      );
    }
    if (sync.phase === "reauth_required") {
      return (
        <View style={styles.actions}>
          <Button
            label="Sign out"
            onPress={() => void auth.signOut()}
            size="small"
            variant="outline"
          />
        </View>
      );
    }
    if (sync.phase === "error" || sync.phase === "device_limit") {
      return (
        <View style={styles.actions}>
          <Button
            label="Try again"
            onPress={retryMobileSync}
            size="small"
            variant="outline"
          />
        </View>
      );
    }
    return null;
  };

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      style={styles.container}
    >
      <ScrollView
        contentContainerStyle={styles.content}
        keyboardShouldPersistTaps="handled"
        showsVerticalScrollIndicator={false}
      >
        <View style={styles.accountCard}>
          <UserAvatar user={auth.session?.user ?? null} />
          <View style={styles.accountIdentity}>
            <Text style={styles.accountLabel}>Account</Text>
            <Text style={styles.email} numberOfLines={1}>
              {auth.bypass ? "Not signed in" : (auth.session?.user.email ?? "")}
            </Text>
          </View>
          <View style={styles.planChip}>
            <Text style={styles.planLabel}>{planLabel}</Text>
          </View>
        </View>

        <View style={styles.notesListCard}>
          <Text style={styles.eyebrow}>Notes list</Text>
          <Text style={styles.notesListDescription}>
            Choose extra details to show on each note.
          </Text>
          <View style={styles.preferenceGroup}>
            <View>
              <Host
                matchContents={{ vertical: true }}
                seedColor={Colors.primary}
                style={styles.preferenceControl}
              >
                <Switch
                  label="Folder"
                  value={sidebarPreferences.showFolder}
                  onValueChange={(value) =>
                    updateSidebarPreference("sidebar_show_folder", value)
                  }
                  testID="sidebar-show-folder"
                />
              </Host>
              <Text style={styles.preferenceDescription}>
                Show the folder above the title.
              </Text>
            </View>
            <View style={styles.preferenceDivider} />
            <View>
              <Host
                matchContents={{ vertical: true }}
                seedColor={Colors.primary}
                style={styles.preferenceControl}
              >
                <Switch
                  label="Tags"
                  value={sidebarPreferences.showTags}
                  onValueChange={(value) =>
                    updateSidebarPreference("sidebar_show_tags", value)
                  }
                  testID="sidebar-show-tags"
                />
              </Host>
              <Text style={styles.preferenceDescription}>
                Show tags under the date and time.
              </Text>
            </View>
          </View>
        </View>

        <View style={styles.syncCard}>
          <View style={styles.statusHeading}>
            <View
              style={[
                styles.statusDot,
                presentation.healthy
                  ? styles.statusDotReady
                  : presentation.retrying || presentation.pending
                    ? styles.statusDotRetrying
                    : styles.statusDotQuiet,
              ]}
            />
            <Text style={styles.eyebrow}>Cloud sync</Text>
          </View>
          <Text style={styles.syncTitle}>{presentation.title}</Text>
          <Text style={styles.syncDescription}>{presentation.description}</Text>
          {presentation.detail && (
            <Text style={styles.syncDetail}>{presentation.detail}</Text>
          )}
          {renderStatusActions()}
        </View>

        {!auth.bypass && (
          <Button
            label="Sign out"
            onPress={() => void auth.signOut()}
            size="small"
            style={styles.signOut}
            variant="ghost"
          />
        )}
      </ScrollView>
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: Colors.background,
  },
  content: {
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.roomy,
  },
  accountCard: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
    padding: Spacing.md,
  },
  accountIdentity: {
    flex: 1,
  },
  accountLabel: {
    ...Typography.captionStrong,
    color: Colors.muted,
  },
  email: {
    marginTop: Spacing.xs,
    ...Typography.section,
    color: Colors.ink,
  },
  planChip: {
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accentSurface,
    paddingHorizontal: Spacing.sm,
    paddingVertical: Spacing.xs,
  },
  planLabel: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  syncCard: {
    marginTop: Spacing.lg,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
    padding: Spacing.md,
  },
  notesListCard: {
    marginTop: Spacing.lg,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
    padding: Spacing.md,
  },
  notesListDescription: {
    marginTop: Spacing.xs,
    ...Typography.body,
    color: Colors.muted,
  },
  preferenceGroup: {
    marginTop: Spacing.md,
    gap: Spacing.md,
  },
  preferenceControl: {
    width: "100%",
  },
  preferenceDescription: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
  preferenceDivider: {
    height: StyleSheet.hairlineWidth,
    backgroundColor: Colors.border,
  },
  statusHeading: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    marginBottom: Spacing.sm,
  },
  statusDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  statusDotReady: {
    backgroundColor: Colors.primary,
  },
  statusDotRetrying: {
    backgroundColor: Colors.alertForeground,
  },
  statusDotQuiet: {
    backgroundColor: Colors.border,
  },
  eyebrow: {
    ...Typography.captionStrong,
    color: Colors.muted,
  },
  syncTitle: {
    ...Typography.section,
    color: Colors.ink,
  },
  syncDescription: {
    marginTop: Spacing.xs,
    ...Typography.body,
    color: Colors.muted,
  },
  syncDetail: {
    marginTop: Spacing.sm,
    ...Typography.caption,
    color: Colors.muted,
  },
  actions: {
    marginTop: Spacing.md,
    flexDirection: "row",
  },
  signOut: {
    marginTop: Spacing.lg,
    alignSelf: "flex-start",
  },
});
