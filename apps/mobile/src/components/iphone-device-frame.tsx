import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { Colors, CornerCurve, Radius } from "@/constants/theme";

const IPHONE_17_PRO_ASPECT_RATIO = 71.9 / 150;
const IPHONE_17_PRO_DEVICE_CORNER_RADIUS_RATIO = 62 / 402;

export function IPhoneDeviceFrame({
  liveActivity,
  width,
}: {
  liveActivity?: ReactNode;
  width: number;
}) {
  const height = width / IPHONE_17_PRO_ASPECT_RATIO;
  const borderWidth = Math.max(1.5, width * 0.029);
  const actionButtonWidth = Math.max(3, width * 0.058);

  return (
    <View
      accessibilityElementsHidden
      importantForAccessibility="no-hide-descendants"
      style={[
        styles.frame,
        {
          width,
          height,
          borderWidth,
          borderRadius: width * IPHONE_17_PRO_DEVICE_CORNER_RADIUS_RATIO,
        },
      ]}
    >
      <View
        style={[
          styles.dynamicIsland,
          {
            top: width * 0.087,
            width: width * 0.346,
            height: width * 0.096,
          },
        ]}
      />
      <View
        style={[
          styles.actionButton,
          {
            left: -actionButtonWidth,
            top: width * 0.327,
            width: actionButtonWidth,
            height: width * 0.269,
          },
        ]}
      />
      {liveActivity && (
        <View
          style={[
            styles.liveActivity,
            {
              bottom: width * 0.23,
              width: width * 0.846,
              height: width * 0.308,
            },
          ]}
        >
          {liveActivity}
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  frame: {
    alignItems: "center",
    borderColor: Colors.ink,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
  },
  dynamicIsland: {
    position: "absolute",
    borderRadius: Radius.pill,
    backgroundColor: Colors.ink,
  },
  actionButton: {
    position: "absolute",
    borderRadius: Radius.pill,
    backgroundColor: Colors.primary,
  },
  liveActivity: {
    position: "absolute",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.ink,
  },
});
