import * as Sentry from "@sentry/react-native";
import { useFonts } from "expo-font";
import { type ErrorBoundaryProps, Stack, usePathname } from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import { StatusBar } from "expo-status-bar";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { StyleSheet, Text, View } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";

import {
  getMobileCaptureActive,
  subscribeMobileCapture,
} from "@/audio/capture-lifecycle";
import { recoverInterruptedRecordings } from "@/audio/recover-recordings";
import { AuthProvider, useAuth } from "@/auth/context";
import { PaywallScreen, SignInScreen } from "@/auth/screens";
import type { SignInMethod } from "@/auth/sign-in";
import { BrandLoadingView } from "@/components/brand-loading-view";
import { Button } from "@/components/ui/button";
import { Colors, Spacing, Typography } from "@/constants/theme";
import { initializeAnalytics, screenAnalytics } from "@/lib/analytics";
import {
  addNavigationBreadcrumb,
  captureOperationalError,
  initializeErrorReporting,
} from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";
import { QuickActionLifecycle } from "@/quick-actions/quick-action-lifecycle";
import { MobileSyncLifecycle } from "@/sync/mobile-sync-lifecycle";
import { initializeWatchConnectivity } from "@/watch-connectivity";

initializeErrorReporting();
initializeWatchConnectivity();
SplashScreen.setOptions({ duration: 300, fade: true });

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
    <>
      <QuickActionLifecycle accountUserId={accountUserId} />
      <Stack
        screenOptions={{
          headerShown: false,
          contentStyle: { backgroundColor: Colors.paper },
        }}
      />
    </>
  );
}

function Gate() {
  const auth = useAuth();
  const captureActive = useSyncExternalStore(
    subscribeMobileCapture,
    getMobileCaptureActive,
    getMobileCaptureActive,
  );
  const [signingIn, setSigningIn] = useState(false);

  const handleSignIn = async (method: SignInMethod) => {
    setSigningIn(true);
    try {
      await auth.signIn(method);
    } catch {
      // AuthProvider reports the actionable failure before rejecting.
    } finally {
      setSigningIn(false);
    }
  };

  if (auth.bypass) return <Screens accountUserId={null} />;

  if (captureActive) {
    const activeSession = auth.session;
    return (
      <>
        {activeSession && (
          <MobileSyncLifecycle
            key={`${activeSession.user.id}:${activeSession.access_token}`}
            accessToken={activeSession.access_token}
            accountUserId={activeSession.user.id}
          />
        )}
        <Screens accountUserId={activeSession?.user.id ?? null} />
      </>
    );
  }

  if (
    auth.status === "loading" ||
    (auth.status === "signed_in" && !auth.billingReady)
  ) {
    return <BrandLoadingView />;
  }

  if (auth.status === "signed_out") {
    return (
      <SignInScreen
        busy={signingIn}
        lastSignInMethod={auth.lastSignInMethod}
        onSignIn={(method) => void handleSignIn(method)}
      />
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

  const session = auth.session;
  if (!session) return null;

  return (
    <>
      <MobileSyncLifecycle
        key={`${session.user.id}:${session.access_token}`}
        accessToken={session.access_token}
        accountUserId={session.user.id}
      />
      <Screens accountUserId={session.user.id} />
    </>
  );
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
  const [fontsLoaded, fontError] = useFonts({
    CaveatSemiBold: require("../../assets/fonts/Caveat-SemiBold.ttf"),
  });
  const [startupAnimationComplete, setStartupAnimationComplete] =
    useState(false);

  if (!startupAnimationComplete || (!fontsLoaded && !fontError)) {
    return (
      <BrandLoadingView
        animated={!startupAnimationComplete}
        onAnimationComplete={() => setStartupAnimationComplete(true)}
      />
    );
  }

  return (
    <GestureHandlerRootView style={styles.root}>
      <AuthProvider>
        <AnalyticsLifecycle />
        <Gate />
        <StatusBar style="dark" />
      </AuthProvider>
    </GestureHandlerRootView>
  );
}

export default Sentry.wrap(RootLayout);

const styles = StyleSheet.create({
  root: {
    flex: 1,
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
