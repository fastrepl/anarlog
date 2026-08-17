import * as Sentry from "@sentry/react-native";
import { type ErrorBoundaryProps, Stack, usePathname } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { useEffect, useRef, useState } from "react";
import { ActivityIndicator, StyleSheet, Text, View } from "react-native";

import { recoverInterruptedRecordings } from "@/audio/recover-recordings";
import { AuthProvider, useAuth } from "@/auth/context";
import { PaywallScreen, SignInScreen } from "@/auth/screens";
import { Button } from "@/components/ui/button";
import { Colors, Spacing, Typography } from "@/constants/theme";
import { initializeAnalytics, screenAnalytics } from "@/lib/analytics";
import {
  addNavigationBreadcrumb,
  captureOperationalError,
  initializeErrorReporting,
} from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";
import { initializeWatchConnectivity } from "@/watch-connectivity";

initializeErrorReporting();
initializeWatchConnectivity();

const routeErrorKeys = new WeakMap<Error, number>();
let nextRouteErrorKey = 0;

function getRouteErrorKey(error: Error) {
  const existing = routeErrorKeys.get(error);
  if (existing !== undefined) {
    return existing;
  }

  nextRouteErrorKey += 1;
  routeErrorKeys.set(error, nextRouteErrorKey);
  return nextRouteErrorKey;
}

function AnalyticsLifecycle() {
  const pathname = usePathname();
  const previousPathRef = useRef<string | null>(null);

  useEffect(() => {
    void initializeAnalytics();
  }, []);

  useEffect(() => {
    const normalizedPathname = pathname.replace(/^\/note\/[^/]+/, "/note/:id");
    screenAnalytics(normalizedPathname);
    addNavigationBreadcrumb(previousPathRef.current, normalizedPathname);
    previousPathRef.current = normalizedPathname;
  }, [pathname]);

  return null;
}

function Screens({ accountUserId }: { accountUserId: string | null }) {
  useMountEffect(() => {
    void recoverInterruptedRecordings(accountUserId).catch((error) => {
      captureOperationalError(error, {
        operation: "recording_recovery",
        level: "warning",
        tags: { stage: "candidate_query" },
      });
    });
  });

  return (
    <Stack
      screenOptions={{
        headerShown: false,
        contentStyle: { backgroundColor: Colors.paper },
      }}
    />
  );
}

function Gate() {
  const auth = useAuth();
  const [signingIn, setSigningIn] = useState(false);

  const handleSignIn = async () => {
    setSigningIn(true);
    try {
      await auth.signIn();
    } catch {
      // AuthProvider reports the actionable failure before rejecting.
    } finally {
      setSigningIn(false);
    }
  };

  if (auth.bypass) return <Screens accountUserId={null} />;

  if (
    auth.status === "loading" ||
    (auth.status === "signed_in" && !auth.billingReady)
  ) {
    return (
      <View style={styles.loading}>
        <ActivityIndicator color={Colors.ink} />
      </View>
    );
  }

  if (auth.status === "signed_out") {
    return (
      <SignInScreen busy={signingIn} onSignIn={() => void handleSignIn()} />
    );
  }

  if (!auth.billing.isPro) {
    return (
      <PaywallScreen
        billing={auth.billing}
        email={auth.session?.user.email ?? ""}
        onRefreshBilling={auth.refreshBilling}
        onSignOut={auth.signOut}
      />
    );
  }

  return <Screens accountUserId={auth.session?.user.id ?? null} />;
}

function RouteError({
  error,
  retry,
}: {
  error: Error;
  retry: () => Promise<void>;
}) {
  useMountEffect(() => {
    captureOperationalError(error, {
      operation: "route_render",
    });
  });

  const handleRetry = async () => {
    try {
      await retry();
    } catch (retryError) {
      captureOperationalError(retryError, {
        operation: "route_retry",
      });
    }
  };

  return (
    <View style={styles.routeError}>
      <Text style={styles.routeErrorTitle}>Something went wrong</Text>
      <Text style={styles.routeErrorBody}>
        Your notes are still stored on this device.
      </Text>
      <Button
        label="Try again"
        onPress={() => void handleRetry()}
        style={styles.routeErrorButton}
      />
    </View>
  );
}

export function ErrorBoundary({ error, retry }: ErrorBoundaryProps) {
  return (
    <RouteError key={getRouteErrorKey(error)} error={error} retry={retry} />
  );
}

function RootLayout() {
  return (
    <AuthProvider>
      <AnalyticsLifecycle />
      <Gate />
      <StatusBar style="dark" />
    </AuthProvider>
  );
}

export default Sentry.wrap(RootLayout);

const styles = StyleSheet.create({
  loading: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: Colors.paper,
  },
  routeError: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.md,
    paddingHorizontal: Spacing.xl,
    backgroundColor: Colors.background,
  },
  routeErrorTitle: {
    ...Typography.title,
    color: Colors.ink,
  },
  routeErrorBody: {
    ...Typography.body,
    textAlign: "center",
    color: Colors.muted,
  },
  routeErrorButton: {
    marginTop: Spacing.sm,
    minWidth: 140,
  },
});
