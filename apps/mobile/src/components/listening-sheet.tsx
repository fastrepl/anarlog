import { Ionicons } from "@expo/vector-icons";
import { useRef, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from "react-native-reanimated";

import { useLiveAudioLevels } from "@/audio/use-live-audio-levels";
import { Waveform, WAVEFORM_BAR_COUNT } from "@/components/waveform";
import { Colors, Radius, Spacing } from "@/constants/theme";
import { useMockLiveTranscript } from "@/data/transcript";

const TRANSCRIPT_HEIGHT = 320;

export function ListeningSheet({ onStop }: { onStop: () => void }) {
  const [expanded, setExpanded] = useState(false);
  const transcriptHeight = useSharedValue(0);
  const scrollRef = useRef<ScrollView>(null);
  const lines = useMockLiveTranscript();
  const levels = useLiveAudioLevels(WAVEFORM_BAR_COUNT);

  const toggle = () => {
    const next = !expanded;
    setExpanded(next);
    transcriptHeight.value = withTiming(next ? TRANSCRIPT_HEIGHT : 0, {
      duration: 240,
    });
  };

  const transcriptStyle = useAnimatedStyle(() => ({
    height: transcriptHeight.value,
  }));

  return (
    <View style={styles.sheet}>
      <Pressable hitSlop={12} onPress={toggle} style={styles.chevron}>
        <Ionicons
          name={expanded ? "chevron-down" : "chevron-up"}
          size={22}
          color={Colors.ink}
        />
      </Pressable>

      <Animated.View style={[styles.transcript, transcriptStyle]}>
        <ScrollView
          ref={scrollRef}
          onContentSizeChange={() =>
            scrollRef.current?.scrollToEnd({ animated: true })
          }
          contentContainerStyle={styles.transcriptContent}
        >
          {lines.map((line) => (
            <View key={line.id} style={styles.line}>
              <Text style={styles.speaker}>{line.speaker}</Text>
              <Text style={styles.words}>{line.text}</Text>
            </View>
          ))}
        </ScrollView>
      </Animated.View>

      <Pressable
        onPress={onStop}
        style={({ pressed }) => [styles.panel, pressed && styles.panelPressed]}
      >
        <Waveform levels={levels} />
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  sheet: {
    borderWidth: 1.5,
    borderBottomWidth: 0,
    borderColor: Colors.ink,
    borderTopLeftRadius: Radius.pill,
    borderTopRightRadius: Radius.pill,
    backgroundColor: Colors.paper,
    paddingHorizontal: Spacing.md,
    paddingBottom: Spacing.md,
  },
  chevron: {
    alignSelf: "center",
    paddingVertical: Spacing.sm,
  },
  transcript: {
    overflow: "hidden",
  },
  transcriptContent: {
    paddingHorizontal: Spacing.sm,
    paddingBottom: Spacing.md,
  },
  line: {
    marginBottom: Spacing.md,
  },
  speaker: {
    fontSize: 13,
    fontWeight: "600",
    color: Colors.muted,
    marginBottom: 2,
  },
  words: {
    fontSize: 16,
    color: Colors.ink,
  },
  panel: {
    borderRadius: Radius.card + 4,
    backgroundColor: Colors.accent,
    paddingVertical: Spacing.md,
  },
  panelPressed: {
    opacity: 0.9,
  },
});
