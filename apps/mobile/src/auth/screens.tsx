import * as WebBrowser from "expo-web-browser";
import {
  ActivityIndicator,
  Pressable,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import type { BillingInfo } from "@/auth/billing";
import { Colors, Radius, Spacing } from "@/constants/theme";
import { captureAnalytics } from "@/lib/analytics";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";

export function SignInScreen({
  onSignIn,
  busy,
}: {
  onSignIn: () => void;
  busy: boolean;
}) {
  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.body}>
        <View style={styles.titleRow}>
          <Text style={styles.title}>Anarlog</Text>
          <View style={styles.accentDot} />
        </View>
        <Text style={styles.subtitle}>Meetings, remembered.</Text>
        <Text style={styles.copy}>
          Sign in to sync your notes and recordings.
        </Text>
      </View>

      <Pressable
        onPress={onSignIn}
        disabled={busy}
        style={({ pressed }) => [
          styles.cta,
          pressed && styles.ctaPressed,
          busy && styles.ctaDisabled,
        ]}
      >
        {busy ? (
          <ActivityIndicator color={Colors.inkInverse} />
        ) : (
          <Text style={styles.ctaLabel}>Sign in</Text>
        )}
      </Pressable>
    </SafeAreaView>
  );
}

export function PaywallScreen({
  billing,
  email,
  onSignOut,
}: {
  billing: BillingInfo;
  email: string;
  onSignOut: () => void;
}) {
  useMountEffect(() => {
    captureAnalytics("paywall_viewed", {
      entry_point: "mobile_gate",
      feature: "mobile_access",
    });
  });

  const handleOpenPlans = async () => {
    try {
      await WebBrowser.openBrowserAsync(
        `${env.appUrl}/app/checkout?source=mobile`,
      );
    } catch (error) {
      captureOperationalError(error, {
        operation: "billing_checkout_open",
        tags: { entry_point: "mobile_gate" },
      });
    }
  };

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.body}>
        <View style={styles.titleRow}>
          <Text style={styles.paywallTitle}>Anarlog Mobile is for Pro</Text>
          <View style={styles.accentDot} />
        </View>
        <Text style={styles.copy}>
          Keep notes end-to-end encrypted and up to date across your signed-in
          devices.
        </Text>
        {billing.plan === "trial" && billing.trialDaysRemaining !== null && (
          <Text style={styles.trialLine}>
            Trial: {billing.trialDaysRemaining} days left
          </Text>
        )}
      </View>

      <Pressable
        onPress={() => void handleOpenPlans()}
        style={({ pressed }) => [styles.cta, pressed && styles.ctaPressed]}
      >
        <Text style={styles.ctaLabel}>View plans</Text>
      </Pressable>

      <View style={styles.footer}>
        <Text style={styles.footerEmail} numberOfLines={1}>
          {email}
        </Text>
        <Pressable onPress={onSignOut} hitSlop={8}>
          <Text style={styles.signOutLabel}>Sign out</Text>
        </Pressable>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.paper,
    paddingHorizontal: Spacing.lg,
  },
  body: {
    flex: 1,
    justifyContent: "center",
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
  },
  title: {
    fontSize: 34,
    fontWeight: "700",
    color: Colors.ink,
  },
  paywallTitle: {
    flexShrink: 1,
    fontSize: 26,
    fontWeight: "700",
    color: Colors.ink,
  },
  accentDot: {
    width: 12,
    height: 12,
    borderRadius: 6,
    backgroundColor: Colors.accent,
  },
  subtitle: {
    marginTop: Spacing.sm,
    fontSize: 18,
    fontWeight: "600",
    color: Colors.ink,
  },
  copy: {
    marginTop: Spacing.md,
    fontSize: 15,
    lineHeight: 22,
    color: Colors.muted,
  },
  trialLine: {
    marginTop: Spacing.md,
    fontSize: 13,
    color: Colors.muted,
  },
  cta: {
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: Spacing.lg,
    borderRadius: Radius.pill,
    backgroundColor: Colors.ink,
    marginBottom: Spacing.md,
  },
  ctaPressed: {
    opacity: 0.85,
  },
  ctaDisabled: {
    opacity: 0.7,
  },
  ctaLabel: {
    fontSize: 18,
    fontWeight: "600",
    color: Colors.inkInverse,
  },
  footer: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: Spacing.md,
    marginBottom: Spacing.lg,
  },
  footerEmail: {
    flexShrink: 1,
    fontSize: 13,
    color: Colors.muted,
  },
  signOutLabel: {
    fontSize: 13,
    fontWeight: "600",
    color: Colors.ink,
  },
});
