import { Image } from "expo-image";
import { StyleSheet, View } from "react-native";

import { Colors, Gradients } from "@/constants/theme";

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
    backgroundColor: Colors.brandBackgroundTop,
    experimental_backgroundImage: Gradients.brandBackground,
  },
  mark: {
    width: 76,
    height: 76,
    opacity: 0.72,
  },
});
