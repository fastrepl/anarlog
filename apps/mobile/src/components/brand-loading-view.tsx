import { Image } from "expo-image";
import { StyleSheet, View } from "react-native";

import { Colors } from "@/constants/theme";

export function BrandLoadingView() {
  return (
    <View
      accessible
      accessibilityLabel="Loading Anarlog"
      accessibilityRole="progressbar"
      style={styles.container}
    >
      <Image
        contentFit="contain"
        source={require("../../assets/images/splash-icon.png")}
        style={styles.mark}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: Colors.paper,
  },
  mark: {
    width: 76,
    height: 76,
    opacity: 0.72,
  },
});
