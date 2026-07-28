import { Stack, usePathname } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { useEffect, useState } from "react";
import { ActivityIndicator, StyleSheet, View } from "react-native";

import { AuthProvider, useAuth } from "@/auth/context";
import { PaywallScreen, SignInScreen } from "@/auth/screens";
import { Colors } from "@/constants/theme";
import { initializeAnalytics, screenAnalytics } from "@/lib/analytics";

function AnalyticsLifecycle() {
  const pathname = usePathname();

  useEffect(() => {
    void initializeAnalytics();
  }, []);

  useEffect(() => {
    screenAnalytics(pathname.replace(/^\/note\/[^/]+/, "/note/:id"));
  }, [pathname]);

  return null;
}

function Screens() {
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

  if (auth.bypass) return <Screens />;

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
      <SignInScreen
        busy={signingIn}
        onSignIn={() => {
          setSigningIn(true);
          auth.signIn().finally(() => setSigningIn(false));
        }}
      />
    );
  }

  if (!auth.billing.isPro) {
    return (
      <PaywallScreen
        billing={auth.billing}
        email={auth.session?.user.email ?? ""}
        onSignOut={auth.signOut}
      />
    );
  }

  return <Screens />;
}

export default function RootLayout() {
  return (
    <AuthProvider>
      <AnalyticsLifecycle />
      <Gate />
      <StatusBar style="dark" />
    </AuthProvider>
  );
}

const styles = StyleSheet.create({
  loading: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: Colors.paper,
  },
});
