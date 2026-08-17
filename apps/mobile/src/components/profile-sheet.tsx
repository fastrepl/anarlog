import { useState, useSyncExternalStore } from "react";
import {
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  ScrollView,
  Share,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";

import { useAuth } from "@/auth/context";
import { Button } from "@/components/ui/button";
import {
  Colors,
  CornerCurve,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";
import {
  confirmMobileRecoveryKey,
  generateMobileRecoveryKey,
  getMobileSyncSnapshot,
  importMobileRecoveryKey,
  retryMobileSync,
  subscribeMobileSync,
  syncMobileNow,
} from "@/sync/mobile-sync";
import { syncStatusPresentation } from "@/sync/status-presentation";

type SheetMode = "status" | "choose" | "import" | "generated";

export function ProfileSheet({
  visible,
  onClose,
}: {
  visible: boolean;
  onClose: () => void;
}) {
  const auth = useAuth();
  const sync = useSyncExternalStore(
    subscribeMobileSync,
    getMobileSyncSnapshot,
    getMobileSyncSnapshot,
  );
  const [mode, setMode] = useState<SheetMode>("status");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [generatedKey, setGeneratedKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const planLabel = auth.bypass
    ? "Local dev"
    : auth.billing.plan === "trial"
      ? `Trial · ${auth.billing.trialDaysRemaining ?? 0}d left`
      : auth.billing.plan === "pro"
        ? "Pro"
        : "Free";
  const presentation = syncStatusPresentation(sync);

  const close = () => {
    if (mode === "generated" || busy) return;
    setMode("status");
    setRecoveryKey("");
    setActionError(null);
    onClose();
  };

  const createRecoveryKey = async () => {
    setBusy(true);
    setActionError(null);
    try {
      const key = await generateMobileRecoveryKey();
      setGeneratedKey(key);
      setMode("generated");
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Could not create the key.",
      );
    } finally {
      setBusy(false);
    }
  };

  const importRecoveryKey = async () => {
    if (!recoveryKey.trim()) {
      setActionError("Enter your recovery key.");
      return;
    }
    setBusy(true);
    setActionError(null);
    try {
      await importMobileRecoveryKey(recoveryKey);
      setRecoveryKey("");
      setMode("status");
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Could not use this key.",
      );
    } finally {
      setBusy(false);
    }
  };

  const shareRecoveryKey = async () => {
    setActionError(null);
    try {
      await Share.share({
        title: "Anarlog recovery key",
        message: `Anarlog recovery key\n\n${generatedKey}`,
      });
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Could not share the key.",
      );
    }
  };

  const finishRecoveryKey = async () => {
    setBusy(true);
    setActionError(null);
    try {
      await confirmMobileRecoveryKey(generatedKey);
      setGeneratedKey("");
      setMode("status");
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Could not protect the key.",
      );
    } finally {
      setBusy(false);
    }
  };

  const discardGeneratedKey = () => {
    setGeneratedKey("");
    setActionError(null);
    setMode("status");
  };

  const renderStatusActions = () => {
    if (auth.bypass || sync.phase === "inactive") return null;
    if (sync.phase === "identity_mismatch") {
      return (
        <View style={styles.actions}>
          <Button
            label="Use recovery key"
            onPress={() => setMode("import")}
            size="small"
          />
        </View>
      );
    }
    if (sync.phase === "setup_required") {
      return (
        <View style={styles.actions}>
          <Button
            label="Set up sync"
            onPress={() => setMode("choose")}
            size="small"
          />
        </View>
      );
    }
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
            onPress={() => {
              close();
              void auth.signOut();
            }}
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
    <Modal
      visible={visible}
      transparent
      animationType="slide"
      onRequestClose={close}
    >
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : undefined}
        style={styles.backdrop}
      >
        <Pressable style={styles.backdropPressable} onPress={close}>
          <Pressable style={styles.sheet} onPress={(e) => e.stopPropagation()}>
            <View style={styles.handle} />
            <ScrollView
              contentContainerStyle={styles.content}
              keyboardShouldPersistTaps="handled"
              showsVerticalScrollIndicator={false}
            >
              <View style={styles.row}>
                <Text style={styles.email} numberOfLines={1}>
                  {auth.bypass
                    ? "Not signed in"
                    : (auth.session?.user.email ?? "")}
                </Text>
                <View style={styles.planChip}>
                  <Text style={styles.planLabel}>{planLabel}</Text>
                </View>
              </View>

              {mode === "status" && (
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
                  <Text style={styles.syncDescription}>
                    {presentation.description}
                  </Text>
                  {presentation.detail && (
                    <Text style={styles.syncDetail}>{presentation.detail}</Text>
                  )}
                  {renderStatusActions()}
                </View>
              )}

              {mode === "choose" && (
                <View style={styles.setup}>
                  <Text style={styles.setupTitle}>Set up encrypted sync</Text>
                  <Text style={styles.setupDescription}>
                    Your recovery key protects every synced note. Anarlog cannot
                    recover it for you.
                  </Text>
                  <Button
                    label="Create a recovery key"
                    loading={busy}
                    onPress={() => void createRecoveryKey()}
                  />
                  <Button
                    label="Use an existing key"
                    disabled={busy}
                    onPress={() => {
                      setActionError(null);
                      setMode("import");
                    }}
                    variant="outline"
                  />
                  <Button
                    label="Back"
                    disabled={busy}
                    onPress={() => setMode("status")}
                    size="small"
                    variant="ghost"
                  />
                </View>
              )}

              {mode === "import" && (
                <View style={styles.setup}>
                  <Text style={styles.setupTitle}>Use a recovery key</Text>
                  <Text style={styles.setupDescription}>
                    Enter the key saved from Anarlog on another device.
                  </Text>
                  <TextInput
                    autoCapitalize="none"
                    autoCorrect={false}
                    editable={!busy}
                    multiline
                    onChangeText={setRecoveryKey}
                    placeholder="Paste recovery key"
                    placeholderTextColor={Colors.muted}
                    style={styles.keyInput}
                    value={recoveryKey}
                  />
                  <Button
                    label="Use this key"
                    loading={busy}
                    onPress={() => void importRecoveryKey()}
                  />
                  <Button
                    label="Back"
                    disabled={busy}
                    onPress={() => {
                      setActionError(null);
                      setMode(
                        sync.phase === "identity_mismatch"
                          ? "status"
                          : "choose",
                      );
                    }}
                    size="small"
                    variant="ghost"
                  />
                </View>
              )}

              {mode === "generated" && (
                <View style={styles.setup}>
                  <Text style={styles.setupTitle}>Save your recovery key</Text>
                  <Text style={styles.setupDescription}>
                    Keep this in a password manager. You will need it to add
                    another device or recover your synced notes.
                  </Text>
                  <View style={styles.generatedKeyBox}>
                    <Text selectable style={styles.generatedKey}>
                      {generatedKey}
                    </Text>
                  </View>
                  <Button
                    label="Save recovery key"
                    onPress={() => void shareRecoveryKey()}
                    variant="outline"
                  />
                  <Button
                    label="I saved it"
                    loading={busy}
                    onPress={() => void finishRecoveryKey()}
                  />
                  <Button
                    label="Cancel"
                    disabled={busy}
                    onPress={discardGeneratedKey}
                    size="small"
                    variant="ghost"
                  />
                </View>
              )}

              {actionError && (
                <Text accessibilityLiveRegion="polite" style={styles.error}>
                  {actionError}
                </Text>
              )}

              {!auth.bypass && mode === "status" && (
                <Button
                  label="Sign out"
                  onPress={() => {
                    close();
                    void auth.signOut();
                  }}
                  size="small"
                  style={styles.signOut}
                  variant="ghost"
                />
              )}
            </ScrollView>
          </Pressable>
        </Pressable>
      </KeyboardAvoidingView>
    </Modal>
  );
}

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
  },
  backdropPressable: {
    flex: 1,
    justifyContent: "flex-end",
    backgroundColor: Colors.scrim,
  },
  sheet: {
    maxHeight: "88%",
    backgroundColor: Colors.surface,
    borderTopLeftRadius: Radius.sheet,
    borderTopRightRadius: Radius.sheet,
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.xl,
    paddingTop: Spacing.sm,
  },
  content: {
    paddingBottom: Spacing.md,
  },
  handle: {
    alignSelf: "center",
    width: 40,
    height: 4,
    borderRadius: 2,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.border,
    marginBottom: Spacing.md,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: Spacing.sm,
  },
  email: {
    flex: 1,
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
    backgroundColor: Colors.background,
    padding: Spacing.md,
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
  setup: {
    marginTop: Spacing.lg,
    gap: Spacing.sm,
  },
  setupTitle: {
    ...Typography.title,
    color: Colors.ink,
  },
  setupDescription: {
    marginBottom: Spacing.sm,
    ...Typography.body,
    color: Colors.muted,
  },
  keyInput: {
    minHeight: 108,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.control,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.background,
    padding: Spacing.md,
    ...Typography.body,
    color: Colors.ink,
    fontFamily: Platform.select({ ios: "Menlo", android: "monospace" }),
    textAlignVertical: "top",
  },
  generatedKeyBox: {
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.control,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.background,
    padding: Spacing.md,
  },
  generatedKey: {
    ...Typography.body,
    color: Colors.ink,
    fontFamily: Platform.select({ ios: "Menlo", android: "monospace" }),
  },
  error: {
    marginTop: Spacing.sm,
    ...Typography.caption,
    color: Colors.destructive,
  },
  signOut: {
    marginTop: Spacing.lg,
    alignSelf: "flex-start",
  },
});
