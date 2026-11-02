import { Text } from "react-native";

import { Spacing, Typography } from "@/constants/theme";
import { createStyleHook } from "@/settings/theme-provider";

export function RecordingInterruptionNotice() {
  const styles = useStyles();
  return (
    <Text accessibilityRole="alert" style={styles.notice}>
      Recording ended unexpectedly. The saved audio includes everything captured
      before the interruption.
    </Text>
  );
}

const useStyles = createStyleHook((Colors) => ({
  notice: {
    ...Typography.caption,
    color: Colors.accent,
    marginHorizontal: Spacing.md,
    marginTop: Spacing.sm,
  },
}));
