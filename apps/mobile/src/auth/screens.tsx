import { BottomSheet, RNHostView } from "@expo/ui";
import { Image } from "expo-image";
import * as WebBrowser from "expo-web-browser";
import { useState } from "react";
import {
  Pressable,
  StyleSheet,
  Text,
  useWindowDimensions,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import type { BillingInfo } from "@/auth/billing";
import {
  buildMobileBillingReturnUrl,
  parseBillingCallbackUrl,
} from "@/auth/billing-handoff";
import type { SignInMethod } from "@/auth/sign-in";
import { Button } from "@/components/ui/button";
import {
  Colors,
  CornerCurve,
  Gradients,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";
import { captureAnalytics } from "@/lib/analytics";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";

export function SignInScreen({
  onSignIn,
  busy,
  lastSignInMethod,
}: {
  onSignIn: (method: SignInMethod) => void;
  busy: boolean;
  lastSignInMethod: SignInMethod | null;
}) {
  const [showSignInMethods, setShowSignInMethods] = useState(false);
  const { width } = useWindowDimensions();

  return (
    <SafeAreaView style={[styles.safeArea, styles.brandBackground]}>
      <View style={styles.signInBrand} testID="sign-in-screen">
        <Image
          contentFit="contain"
          source={require("../../assets/images/anarlog-wordmark.png")}
          style={styles.wordmark}
        />
        <Text style={styles.signInTitle}>
          The AI notepad for{"\n"}private meetings.
        </Text>
      </View>

      <Button
        label="Sign in"
        onPress={() => setShowSignInMethods(true)}
        disabled={busy}
        loading={busy}
        size="large"
        style={styles.cta}
      />

      <BottomSheet
        isPresented={showSignInMethods}
        onDismiss={() => setShowSignInMethods(false)}
        testID="sign-in-methods"
      >
        <RNHostView matchContents>
          <View style={[styles.signInMethodList, { width }]}>
            <SignInMethodButton
              method="apple"
              label="Sign in with Apple"
              onSignIn={onSignIn}
              disabled={busy}
              iconSource={require("../../assets/images/auth/apple.svg")}
              lastSignInMethod={lastSignInMethod}
            />
            <SignInMethodButton
              method="google"
              label="Sign in with Google"
              onSignIn={onSignIn}
              disabled={busy}
              iconSource={require("../../assets/images/auth/google.svg")}
              lastSignInMethod={lastSignInMethod}
            />
            <SignInMethodButton
              method="azure"
              label="Sign in with Microsoft"
              onSignIn={onSignIn}
              disabled={busy}
              iconSource={require("../../assets/images/auth/microsoft.svg")}
              lastSignInMethod={lastSignInMethod}
            />
            <SignInMethodButton
              method="github"
              label="Sign in with GitHub"
              onSignIn={onSignIn}
              disabled={busy}
              iconSource={require("../../assets/images/auth/github.svg")}
              lastSignInMethod={lastSignInMethod}
            />
            <SignInMethodButton
              method="email"
              label="Sign in with Email"
              onSignIn={onSignIn}
              disabled={busy}
              iconSource={require("../../assets/images/auth/email.svg")}
              lastSignInMethod={lastSignInMethod}
            />
            <SignInMethodButton
              method="sso"
              label="Sign in with SSO"
              onSignIn={onSignIn}
              disabled={busy}
              iconSource={require("../../assets/images/auth/sso.svg")}
              lastSignInMethod={lastSignInMethod}
            />
            <Text style={styles.legalNotice}>
              By signing up, you agree to our{" "}
              <Text
                accessibilityRole="link"
                onPress={() =>
                  void WebBrowser.openBrowserAsync("https://anarlog.so/terms")
                }
                style={styles.legalLink}
              >
                Terms of Service
              </Text>{" "}
              and{" "}
              <Text
                accessibilityRole="link"
                onPress={() =>
                  void WebBrowser.openBrowserAsync("https://anarlog.so/privacy")
                }
                style={styles.legalLink}
              >
                Privacy Policy
              </Text>
              .
            </Text>
          </View>
        </RNHostView>
      </BottomSheet>
    </SafeAreaView>
  );
}

function SignInMethodButton({
  method,
  label,
  iconSource,
  onSignIn,
  disabled,
  lastSignInMethod,
}: {
  method: SignInMethod;
  label: string;
  iconSource: number;
  onSignIn: (method: SignInMethod) => void;
  disabled: boolean;
  lastSignInMethod: SignInMethod | null;
}) {
  const isLastUsed = method === lastSignInMethod;

  return (
    <View
      style={[
        styles.signInMethodContainer,
        isLastUsed && styles.signInMethodLastUsed,
      ]}
    >
      <Button
        label={label}
        onPress={() => onSignIn(method)}
        disabled={disabled}
        leading={<ProviderIcon source={iconSource} />}
        variant="outline"
        style={styles.signInMethod}
      />
      {isLastUsed && (
        <View
          pointerEvents="none"
          style={styles.lastUsedBadge}
          testID={`last-used-${method}`}
        >
          <Text style={styles.lastUsedLabel}>Last used</Text>
        </View>
      )}
    </View>
  );
}

function ProviderIcon({ source }: { source: number }) {
  return (
    <Image contentFit="contain" source={source} style={styles.providerIcon} />
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
        `${env.appUrl.replace(/\/+$/, "")}/app/checkout?period=monthly&source=mobile&scheme=${env.appScheme}`,
        buildMobileBillingReturnUrl(env.appScheme),
      );
      if (result.type !== "success") return;

      const callback = parseBillingCallbackUrl(result.url, env.appScheme);
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
          Record in-person meetings and voice notes from your phone, then keep
          notes and transcripts in sync with Anarlog on your other devices.
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

      <Button
        label={accessPending ? "Refresh access" : "View plans"}
        onPress={() => void handlePrimaryAction()}
        disabled={busy}
        loading={busy}
        size="large"
        style={styles.cta}
      />

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
    backgroundColor: Colors.background,
    paddingHorizontal: Spacing.lg,
  },
  brandBackground: {
    backgroundColor: Colors.brandBackgroundTop,
    experimental_backgroundImage: Gradients.brandBackground,
  },
  body: {
    flex: 1,
    justifyContent: "center",
  },
  signInBrand: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.lg,
  },
  wordmark: {
    width: 200,
    height: 56,
  },
  signInTitle: {
    maxWidth: 340,
    color: Colors.ink,
    fontFamily: "CaveatSemiBold",
    fontSize: 38,
    lineHeight: 44,
    textAlign: "center",
  },
  signInMethodList: {
    gap: Spacing.sm,
    paddingHorizontal: Spacing.md,
    paddingBottom: Spacing.md,
  },
  signInMethodContainer: {
    position: "relative",
    width: "100%",
  },
  signInMethodLastUsed: {
    paddingTop: Spacing.xs,
  },
  signInMethod: {
    width: "100%",
  },
  lastUsedBadge: {
    position: "absolute",
    top: 0,
    right: Spacing.md,
    paddingHorizontal: Spacing.sm,
    paddingVertical: 1,
    borderWidth: 2,
    borderColor: Colors.surface,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.ink,
  },
  lastUsedLabel: {
    ...Typography.captionStrong,
    color: Colors.inkInverse,
  },
  providerIcon: {
    width: 18,
    height: 18,
  },
  legalNotice: {
    alignSelf: "center",
    maxWidth: 320,
    marginTop: Spacing.sm,
    ...Typography.caption,
    color: Colors.muted,
    textAlign: "center",
  },
  legalLink: {
    color: Colors.muted,
    textDecorationLine: "underline",
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
  },
  paywallTitle: {
    flexShrink: 1,
    ...Typography.title,
    color: Colors.ink,
  },
  accentDot: {
    width: 12,
    height: 12,
    borderRadius: 6,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accent,
  },
  copy: {
    marginTop: Spacing.md,
    ...Typography.body,
    color: Colors.muted,
  },
  trialLine: {
    marginTop: Spacing.md,
    ...Typography.caption,
    color: Colors.muted,
  },
  pendingCopy: {
    marginTop: Spacing.md,
    ...Typography.caption,
    color: Colors.alertForeground,
  },
  cta: {
    marginBottom: Spacing.md,
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
    ...Typography.caption,
    color: Colors.muted,
  },
  signOutLabel: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
});
