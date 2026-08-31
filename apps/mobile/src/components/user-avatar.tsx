import { Ionicons } from "@expo/vector-icons";
import type { User } from "@supabase/supabase-js";
import { Image } from "expo-image";
import { useState } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";

import {
  getProviderProfileImageUrl,
  getProviderProfileName,
} from "@anlg/supabase/profile";

import { Colors, CornerCurve, Radius, Typography } from "@/constants/theme";

function profileInitials(name: string | null): string {
  if (!name) return "";

  const value = name.includes("@") ? (name.split("@")[0] ?? name) : name;
  return value
    .trim()
    .split(/[\s._-]+/)
    .map((part) => Array.from(part)[0] ?? "")
    .filter(Boolean)
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

export function UserAvatar({
  size = 40,
  user,
}: {
  size?: number;
  user: User | null;
}) {
  const imageUrl = getProviderProfileImageUrl(user);
  const initials = profileInitials(getProviderProfileName(user));

  return (
    <View
      style={[
        styles.avatar,
        { borderRadius: size / 2, height: size, width: size },
      ]}
    >
      {initials ? (
        <Text style={[styles.initials, { fontSize: size * 0.36 }]}>
          {initials}
        </Text>
      ) : (
        <Ionicons name="person-outline" size={size * 0.55} color={Colors.ink} />
      )}
      {imageUrl ? <ProviderImage key={imageUrl} imageUrl={imageUrl} /> : null}
    </View>
  );
}

function ProviderImage({ imageUrl }: { imageUrl: string }) {
  const [failed, setFailed] = useState(false);
  if (failed) return null;

  return (
    <Image
      cachePolicy="memory-disk"
      contentFit="cover"
      onError={() => setFailed(true)}
      source={{ uri: imageUrl }}
      style={StyleSheet.absoluteFill}
    />
  );
}

export function UserAvatarButton({
  accessibilityLabel,
  onPress,
  user,
}: {
  accessibilityLabel: string;
  onPress: () => void;
  user: User | null;
}) {
  return (
    <Pressable
      accessibilityLabel={accessibilityLabel}
      accessibilityRole="button"
      hitSlop={4}
      onPress={onPress}
      style={({ pressed }) => [styles.button, pressed && styles.pressed]}
    >
      <UserAvatar user={user} />
    </Pressable>
  );
}

const styles = StyleSheet.create({
  avatar: {
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accentSurface,
  },
  initials: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  button: {
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
  },
  pressed: {
    opacity: 0.72,
  },
});
