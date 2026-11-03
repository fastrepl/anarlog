import {
  Platform,
  Text as NativeText,
  View,
  type TextStyle,
} from "react-native";

import { Spacing, Typography } from "@/constants/theme";
import { Text } from "@/i18n";
import {
  parseMarkdownBlocks,
  type InlineSpan,
  type MarkdownBlock,
} from "@/lib/markdown-blocks";
import { createStyleHook } from "@/settings/theme-provider";

export function MarkdownView({
  markdown,
  onLinkPress,
}: {
  markdown: string;
  onLinkPress?: (href: string) => void;
}) {
  const styles = useStyles();
  const blocks = parseMarkdownBlocks(markdown);

  const renderSpans = (spans: InlineSpan[]) =>
    spans.map((span, index) => (
      <Text
        key={index}
        onPress={
          span.href && onLinkPress ? () => onLinkPress(span.href!) : undefined
        }
        style={[
          span.bold && styles.bold,
          span.italic && styles.italic,
          span.code && styles.code,
          span.href && styles.link,
        ]}
      >
        {span.text}
      </Text>
    ));

  const renderBlock = (block: MarkdownBlock, index: number) => {
    if (block.type === "heading") {
      return (
        <Text
          key={index}
          selectable
          style={[
            styles.heading,
            block.level <= 2 ? styles.headingLarge : styles.headingSmall,
            index === 0 && styles.first,
          ]}
        >
          {renderSpans(block.spans)}
        </Text>
      );
    }
    if (block.type === "list") {
      return (
        <View key={index} style={styles.list}>
          {block.items.map((item, itemIndex) => (
            <View
              key={itemIndex}
              style={[styles.item, { paddingLeft: item.depth * Spacing.md }]}
            >
              <Text style={styles.marker}>
                {item.checked !== undefined
                  ? item.checked
                    ? "☑"
                    : "☐"
                  : item.number !== undefined
                    ? `${item.number}.`
                    : item.depth > 0
                      ? "◦"
                      : "•"}
              </Text>
              <Text selectable style={styles.itemText}>
                {renderSpans(item.spans)}
              </Text>
            </View>
          ))}
        </View>
      );
    }
    return (
      <Text key={index} selectable style={styles.paragraph}>
        {renderSpans(block.spans)}
      </Text>
    );
  };

  return <View style={styles.container}>{blocks.map(renderBlock)}</View>;
}

const useStyles = createStyleHook((Colors) => ({
  container: { gap: Spacing.sm },
  first: { marginTop: 0 },
  heading: { color: Colors.ink, marginTop: Spacing.sm },
  headingLarge: { ...Typography.section },
  headingSmall: { ...Typography.bodyStrong },
  paragraph: { ...Typography.body, color: Colors.ink },
  list: { gap: Spacing.xs },
  item: { flexDirection: "row", gap: Spacing.sm },
  marker: {
    ...Typography.body,
    color: Colors.muted,
    minWidth: 18,
    textAlign: "right",
  },
  itemText: { ...Typography.body, color: Colors.ink, flex: 1 },
  bold: { fontWeight: "600" } satisfies TextStyle,
  italic: { fontStyle: "italic" } satisfies TextStyle,
  code: {
    fontFamily: Platform.select({ ios: "Menlo", default: "monospace" }),
    backgroundColor: Colors.mutedSurface,
  } satisfies TextStyle,
  link: {
    color: Colors.accent,
    textDecorationLine: "underline",
  } satisfies TextStyle,
}));
