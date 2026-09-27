import { Ionicons } from "@expo/vector-icons";
import { useRouter } from "expo-router";
import type { ReactNode } from "react";
import { ScrollView, Text, View } from "react-native";
import {
  SafeAreaView,
  useSafeAreaInsets,
} from "react-native-safe-area-context";

import { useMicrophonePermission } from "@/audio/microphone-permission";
import { DancingSticks } from "@/components/dancing-sticks";
import { IPhoneDeviceFrame } from "@/components/iphone-device-frame";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { IconButton } from "@/components/ui/icon-button";
import { ControlSize, Radius, Spacing, Typography } from "@/constants/theme";
import {
  dismissActionButtonCard,
  useActionButtonSetup,
} from "@/quick-actions/action-button-setup";
import { createStyleHook, useColors } from "@/settings/theme-provider";

import { AnarlogShortcutsButton } from "../../modules/quick-actions";

export default function ActionButtonScreen() {
  const styles = useStyles();
  const Colors = useColors();
  const router = useRouter();
  const insets = useSafeAreaInsets();
  const { permission, request } = useMicrophonePermission();
  const setup = useActionButtonSetup();
  const microphoneAllowed = permission.data?.granted === true;
  const shortcutRan = setup.data?.shortcutRan === true;

  const handleBack = () => {
    if (router.canGoBack()) router.back();
    else router.replace("/");
  };

  const handleDone = () => {
    void dismissActionButtonCard();
    handleBack();
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
          <SetupStep
            number={1}
            done={microphoneAllowed}
            title="Allow microphone access"
            description={
              microphoneAllowed
                ? "Anarlog can start listening as soon as it opens."
                : "Anarlog needs the microphone to record when you press the button."
            }
          >
            {!microphoneAllowed && permission.data && (
              <Button
                label={
                  permission.data.canAskAgain
                    ? "Allow Microphone"
                    : "Open Settings"
                }
                loading={request.isPending}
                onPress={() => request.mutate()}
                size="small"
                style={styles.stepAction}
                variant="outline"
              />
            )}
          </SetupStep>
          <SetupStep
            number={2}
            done={false}
            title="Assign Start Listening"
            description="In iPhone Settings, tap Action Button, swipe to Shortcut, tap Choose a Shortcut, then pick Anarlog › Start Listening."
          >
            <AnarlogShortcutsButton style={styles.shortcutsButton} />
          </SetupStep>
          <SetupStep
            number={3}
            done={shortcutRan}
            title="Try it"
            description={
              shortcutRan
                ? "Start Listening works. Press and hold the Action Button to start listening, and again to stop."
                : "Press and hold the Action Button. Anarlog opens and starts listening; press and hold again to stop."
            }
          />
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
        <Button label="Done" onPress={handleDone} />
      </View>
    </SafeAreaView>
  );
}

function SetupStep({
  children,
  description,
  done,
  number,
  title,
}: {
  children?: ReactNode;
  description: string;
  done: boolean;
  number: number;
  title: string;
}) {
  const styles = useStyles();
  const Colors = useColors();
  return (
    <View
      accessibilityLabel={done ? `${title}, done` : undefined}
      style={styles.step}
    >
      <View style={[styles.stepNumber, done && styles.stepNumberDone]}>
        {done ? (
          <Ionicons
            name="checkmark"
            size={16}
            color={Colors.primaryForeground}
          />
        ) : (
          <Text style={styles.stepNumberLabel}>{number}</Text>
        )}
      </View>
      <View style={styles.stepCopy}>
        <Text style={styles.stepTitle}>{title}</Text>
        <Text style={styles.stepDescription}>{description}</Text>
        {children}
      </View>
    </View>
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
  shortcutsButton: {
    height: ControlSize.default,
    alignSelf: "stretch",
    marginTop: Spacing.sm,
  },
  stepAction: {
    alignSelf: "flex-start",
    marginTop: Spacing.sm,
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
  stepNumberDone: {
    backgroundColor: Colors.primary,
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
