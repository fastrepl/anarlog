import { Ionicons } from "@expo/vector-icons";
import { useRouter } from "expo-router";
import { ScrollView, Text, View } from "react-native";
import {
  SafeAreaView,
  useSafeAreaInsets,
} from "react-native-safe-area-context";

import { DancingSticks } from "@/components/dancing-sticks";
import { IPhoneDeviceFrame } from "@/components/iphone-device-frame";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { IconButton } from "@/components/ui/icon-button";
import { ControlSize, Radius, Spacing, Typography } from "@/constants/theme";
import { createStyleHook, useColors } from "@/settings/theme-provider";

const steps = [
  {
    title: "Open Action Button settings",
    description: "In iPhone Settings, tap Action Button.",
  },
  {
    title: "Choose Shortcut",
    description: "Swipe to Shortcut, then tap Choose a Shortcut.",
  },
  {
    title: "Pick Anarlog",
    description: "Choose Anarlog, then Start Listening.",
  },
];

export default function ActionButtonScreen() {
  const styles = useStyles();
  const Colors = useColors();
  const router = useRouter();
  const insets = useSafeAreaInsets();

  const handleBack = () => {
    if (router.canGoBack()) router.back();
    else router.replace("/");
  };

  return (
    <SafeAreaView edges={["top", "left", "right"]} style={styles.safeArea}>
      <View style={styles.header}>
        <IconButton
          accessibilityLabel="Back"
          icon="back"
          iconSize={22}
          onPress={handleBack}
        />
        <Text style={styles.headerTitle}>Action Button</Text>
        <View style={styles.headerSpacer} />
      </View>
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={[
          styles.content,
          {
            paddingBottom:
              ControlSize.default + Spacing.lg + insets.bottom + Spacing.xs,
          },
        ]}
        contentInsetAdjustmentBehavior="automatic"
      >
        <View style={styles.hero}>
          <IPhoneDeviceFrame
            width={104}
            liveActivity={
              <DancingSticks
                amplitude={1}
                color={Colors.inkInverse}
                gap={2}
                height={18}
                stickWidth={2}
                width={52}
              />
            }
          />
          <Text style={styles.title}>Start listening in one press</Text>
          <Text style={styles.description}>
            Your iPhone Action Button can start a new Anarlog recording or stop
            the one already in progress.
          </Text>
        </View>

        <Card style={styles.stepsCard}>
          {steps.map((step, index) => (
            <View key={step.title} style={styles.step}>
              <View style={styles.stepNumber}>
                <Text style={styles.stepNumberLabel}>{index + 1}</Text>
              </View>
              <View style={styles.stepCopy}>
                <Text style={styles.stepTitle}>{step.title}</Text>
                <Text style={styles.stepDescription}>{step.description}</Text>
              </View>
            </View>
          ))}
        </Card>

        <View style={styles.usage}>
          <Ionicons
            name="radio-button-off-outline"
            size={18}
            color={Colors.primary}
          />
          <Text style={styles.usageLabel}>Long press</Text>
          <Ionicons name="arrow-forward" size={16} color={Colors.muted} />
          <Ionicons name="mic-outline" size={18} color={Colors.primary} />
          <Text style={styles.usageLabel}>Listen</Text>
          <Ionicons name="arrow-forward" size={16} color={Colors.muted} />
          <Ionicons
            name="radio-button-off-outline"
            size={18}
            color={Colors.primary}
          />
          <Text style={styles.usageLabel}>Long press</Text>
        </View>
      </ScrollView>
      <View
        style={[styles.footer, { paddingBottom: insets.bottom + Spacing.xs }]}
      >
        <Button label="Done" onPress={handleBack} />
      </View>
    </SafeAreaView>
  );
}

const useStyles = createStyleHook((Colors) => ({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.background,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: Spacing.lg,
    paddingVertical: Spacing.md,
  },
  headerTitle: {
    ...Typography.section,
    color: Colors.ink,
  },
  headerSpacer: {
    width: ControlSize.default,
    height: ControlSize.default,
  },
  scroll: {
    flex: 1,
  },
  content: {
    paddingHorizontal: Spacing.lg,
  },
  hero: {
    alignItems: "center",
    paddingTop: Spacing.lg,
  },
  title: {
    marginTop: Spacing.lg,
    ...Typography.title,
    color: Colors.ink,
    textAlign: "center",
  },
  description: {
    marginTop: Spacing.sm,
    ...Typography.body,
    color: Colors.muted,
    textAlign: "center",
  },
  stepsCard: {
    marginTop: Spacing.xl,
    padding: Spacing.md,
    gap: Spacing.md,
  },
  step: {
    flexDirection: "row",
    gap: Spacing.sm,
  },
  stepNumber: {
    width: 28,
    height: 28,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.pill,
    backgroundColor: Colors.accentSurface,
  },
  stepNumberLabel: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  stepCopy: {
    flex: 1,
  },
  stepTitle: {
    ...Typography.bodyStrong,
    color: Colors.ink,
  },
  stepDescription: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
  usage: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    flexWrap: "wrap",
    gap: Spacing.xs,
    marginVertical: Spacing.xl,
  },
  usageLabel: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  footer: {
    position: "absolute",
    right: 0,
    bottom: 0,
    left: 0,
    paddingHorizontal: Spacing.lg,
    paddingTop: Spacing.sm,
    backgroundColor: Colors.background,
  },
}));
