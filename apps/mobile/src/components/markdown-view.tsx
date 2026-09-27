import { Platform, Text, View, type TextStyle } from "react-native";

import { Spacing, Typography } from "@/constants/theme";
import {
  parseMarkdownBlocks,
  type InlineSpan,
  type MarkdownBlock,
} from "@/lib/markdown-blocks";
import { createStyleHook } from "@/settings/theme-provider";

export function MarkdownView({ markdown }: { markdown: string }) {
  const styles = useStyles();
  const blocks = parseMarkdownBlocks(markdown);

  const renderSpans = (spans: InlineSpan[]) =>
    spans.map((span, index) => (
      <Text
        key={index}
        style={[
          span.bold && styles.bold,
          span.italic && styles.italic,
          span.code && styles.code,
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
            <View key={itemIndex} style={styles.item}>
              <Text style={styles.marker}>
                {item.checked !== undefined
                  ? item.checked
                    ? "☑"
                    : "☐"
                  : block.ordered
                    ? `${itemIndex + 1}.`
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
}));
