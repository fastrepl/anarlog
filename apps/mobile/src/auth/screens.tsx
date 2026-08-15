import * as WebBrowser from "expo-web-browser";
import { useState } from "react";
import {
  ActivityIndicator,
  Pressable,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import type { BillingInfo } from "@/auth/billing";
import {
  MOBILE_BILLING_RETURN_URL,
  parseBillingCallbackUrl,
} from "@/auth/billing-handoff";
import { Colors, CornerCurve, Radius, Spacing } from "@/constants/theme";
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
          Sign in to use the mobile companion with your Pro account.
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
  onRefreshBilling,
  onSignOut,
}: {
  billing: BillingInfo;
  email: string;
  onRefreshBilling: () => Promise<boolean>;
  onSignOut: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [accessPending, setAccessPending] = useState(false);

  useMountEffect(() => {
    captureAnalytics("paywall_viewed", {
      entry_point: "mobile_gate",
      feature: "mobile_access",
    });
  });

  const refreshAccess = async (checkoutType: "trial" | "paid" | "unknown") => {
    const unlocked = await onRefreshBilling();
    if (unlocked) {
      captureAnalytics("mobile_access_unlocked", {
        entry_point: "mobile_checkout",
        checkout_type: checkoutType,
      });
    } else {
      setAccessPending(true);
      captureAnalytics("billing_refresh_pending", {
        entry_point: "mobile_checkout",
        checkout_type: checkoutType,
      });
    }
  };

  const handlePrimaryAction = async () => {
    if (busy) return;
    setBusy(true);
    try {
      if (accessPending) {
        await refreshAccess("unknown");
        return;
      }

      captureAnalytics("upgrade_clicked", {
        plan: "pro",
        period: "monthly",
        source: "mobile",
      });
      const result = await WebBrowser.openAuthSessionAsync(
        `${env.appUrl.replace(/\/+$/, "")}/app/checkout?period=monthly&source=mobile&scheme=anarlog`,
        MOBILE_BILLING_RETURN_URL,
      );
      if (result.type !== "success") return;

      const callback = parseBillingCallbackUrl(result.url);
      if (!callback) {
        throw new Error("Invalid billing callback URL");
      }
      if (callback.checkout === "canceled" || callback.checkout === "failed") {
        captureAnalytics(`checkout_${callback.checkout}`, {
          checkout_type: callback.checkoutType ?? "unknown",
          entry_source: callback.source,
        });
        return;
      }

      const checkoutType = callback.checkout ?? "unknown";
      captureAnalytics("checkout_returned", {
        checkout_type: checkoutType,
        entry_source: callback.source,
      });
      await refreshAccess(checkoutType);
    } catch (error) {
      captureOperationalError(error, {
        operation: "billing_checkout_open",
        tags: { entry_point: "mobile_gate" },
      });
    } finally {
      setBusy(false);
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
          Record in-person meetings and voice notes from your phone. Notes stay
          on this device while mobile sync is in development.
        </Text>
        {accessPending && (
          <Text style={styles.pendingCopy}>
            Your plan changed, but mobile access is still updating.
          </Text>
        )}
        {billing.plan === "trial" && billing.trialDaysRemaining !== null && (
          <Text style={styles.trialLine}>
            Trial: {billing.trialDaysRemaining} days left
          </Text>
        )}
      </View>

      <Pressable
        onPress={() => void handlePrimaryAction()}
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
          <Text style={styles.ctaLabel}>
            {accessPending ? "Refresh access" : "View plans"}
          </Text>
        )}
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
    borderCurve: CornerCurve.squircle,
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
  pendingCopy: {
    marginTop: Spacing.md,
    fontSize: 13,
    lineHeight: 19,
    color: Colors.accent,
  },
  cta: {
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: Spacing.lg,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
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
