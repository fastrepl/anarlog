import * as Sentry from "@sentry/react-native";
import { useMutation, QueryClientProvider } from "@tanstack/react-query";
import { useFonts } from "expo-font";
import { type ErrorBoundaryProps, Stack, usePathname } from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import { StatusBar } from "expo-status-bar";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { Text, View } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";

import {
  getMobileCaptureActive,
  subscribeMobileCapture,
} from "@/audio/capture-lifecycle";
import { recoverInterruptedRecordings } from "@/audio/recover-recordings";
import { AuthProvider, useAuth } from "@/auth/context";
import { SignInScreen } from "@/auth/screens";
import { useTrial } from "@/auth/use-trial";
import { BrandLoadingView } from "@/components/brand-loading-view";
import { ToastHost } from "@/components/toast-host";
import { Button } from "@/components/ui/button";
import { Spacing, Typography } from "@/constants/theme";
import { initializeAnalytics, screenAnalytics } from "@/lib/analytics";
import {
  addNavigationBreadcrumb,
  captureOperationalError,
  initializeErrorReporting,
} from "@/lib/error-reporting";
import { queryClient } from "@/lib/query-client";
import { useMountEffect } from "@/lib/use-mount-effect";
import { QuickActionLifecycle } from "@/quick-actions/quick-action-lifecycle";
import { AppLock } from "@/settings/app-lock";
import { createStyleHook, useColors } from "@/settings/theme-provider";
import { ThemeProvider, useAppColorScheme } from "@/settings/theme-provider";
import { MobileSyncLifecycle } from "@/sync/mobile-sync-lifecycle";
import { useCloudSyncOptIn } from "@/sync/opt-in";
import { initializeWatchConnectivity } from "@/watch-connectivity";

void initializeErrorReporting().catch(() => {});
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

  useMountEffect(() => {
    void initializeAnalytics();
  });

  useEffect(() => {
    const normalizedPathname = pathname.replace(/^\/note\/[^/]+/, "/note/:id");
    screenAnalytics(normalizedPathname);
    addNavigationBreadcrumb(previousPathRef.current, normalizedPathname);
    previousPathRef.current = normalizedPathname;
  }, [pathname]);

  return null;
}

function Screens({ accountUserId }: { accountUserId: string | null }) {
  const Colors = useColors();
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
  useTrial();
  const captureActive = useSyncExternalStore(
    subscribeMobileCapture,
    getMobileCaptureActive,
    getMobileCaptureActive,
  );
  const signIn = useMutation({ mutationFn: auth.signIn });
  const cloudSyncEnabled = useCloudSyncOptIn(auth.session?.user.id ?? null);

  if (auth.bypass) return <Screens accountUserId={null} />;

  if (
    !captureActive &&
    (auth.status === "loading" ||
      (auth.status === "signed_in" && !auth.billingReady))
  ) {
    return <BrandLoadingView />;
  }

  if (!captureActive && auth.status === "signed_out") {
    return (
      <SignInScreen
        busy={signIn.isPending}
        lastSignInMethod={auth.lastSignInMethod}
        onSignIn={(method) => signIn.mutate(method)}
      />
    );
  }

  const session = auth.session;

  return (
    <>
      {session && auth.billing.isPro && (
        <MobileSyncLifecycle
          key={`${session.user.id}:${session.access_token}:${cloudSyncEnabled}`}
          accessToken={session.access_token}
          accountUserId={session.user.id}
          syncEnabled={cloudSyncEnabled}
        />
      )}
      <Screens accountUserId={session?.user.id ?? null} />
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
  const styles = useStyles();
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
  const styles = useStyles();
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
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <AuthProvider>
            <AnalyticsLifecycle />
            <Gate />
            <ToastHost />
            <AppLock />
            <AppStatusBar />
          </AuthProvider>
        </ThemeProvider>
      </QueryClientProvider>
    </GestureHandlerRootView>
  );
}

function AppStatusBar() {
  return (
    <StatusBar style={useAppColorScheme() === "dark" ? "light" : "dark"} />
  );
}

export default Sentry.wrap(RootLayout);

const useStyles = createStyleHook((Colors) => ({
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
}));
