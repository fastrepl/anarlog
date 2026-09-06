import { Host } from "@expo/ui";
import {
  BottomSheet,
  Button,
  Divider,
  HStack,
  Image,
  Text,
  VStack,
} from "@expo/ui/swift-ui";
import {
  buttonStyle,
  contentShape,
  font,
  foregroundStyle,
  frame,
  padding,
  presentationDragIndicator,
  shapes,
} from "@expo/ui/swift-ui/modifiers";
import { useRef } from "react";

import { Spacing } from "@/constants/theme";
import { useAppColorScheme, useColors } from "@/settings/theme-provider";

import type { NoteActionsSheetProps } from "./note-actions-sheet";

export function NoteActionsSheet({
  hasRecordingHistory,
  listening,
  onClose,
  onDelete,
  onExport,
  onImportRecording,
  onToggleListening,
  visible,
}: NoteActionsSheetProps) {
  const Colors = useColors();
  const colorScheme = useAppColorScheme();
  const pendingAction = useRef<(() => void) | null>(null);
  const actions = [
    { label: "Export", icon: "square.and.arrow.up", onPress: onExport },
    ...(!listening && !hasRecordingHistory
      ? [
          {
            label: "Import recording",
            icon: "icloud.and.arrow.up" as const,
            onPress: onImportRecording,
          },
        ]
      : []),
    ...(listening || !hasRecordingHistory
      ? [
          {
            label: listening ? "Stop listening" : "Start listening",
            icon: listening ? ("mic.slash" as const) : ("mic" as const),
            onPress: onToggleListening,
          },
        ]
      : []),
    { label: "Delete", icon: "trash", onPress: onDelete, destructive: true },
  ] as const;

  return (
    <Host
      style={{ position: "absolute" }}
      pointerEvents="none"
      colorScheme={colorScheme}
    >
      <BottomSheet
        isPresented={visible}
        fitToContents
        onIsPresentedChange={(presented) => {
          if (!presented) onClose();
        }}
        onDismiss={() => {
          // Native share and document pickers need the presenting sheet fully dismissed.
          const action = pendingAction.current;
          pendingAction.current = null;
          action?.();
        }}
      >
        <VStack
          spacing={0}
          modifiers={[
            frame({ maxWidth: Infinity }),
            padding({
              horizontal: Spacing.md,
              top: Spacing.lg,
              bottom: Spacing.sm,
            }),
            presentationDragIndicator("visible"),
          ]}
        >
          {actions.map((action) => {
            const destructive = "destructive" in action;
            return (
              <VStack key={action.label} spacing={0}>
                {destructive && (
                  <Divider modifiers={[padding({ vertical: Spacing.xs })]} />
                )}
                <Button
                  role={destructive ? "destructive" : "default"}
                  modifiers={[buttonStyle("plain")]}
                  onPress={() => {
                    if (pendingAction.current) return;
                    pendingAction.current = action.onPress;
                    onClose();
                  }}
                >
                  <HStack
                    spacing={Spacing.md}
                    modifiers={[
                      padding({ horizontal: Spacing.md, vertical: Spacing.sm }),
                      frame({
                        minHeight: 52,
                        maxWidth: Infinity,
                        alignment: "leading",
                      }),
                      contentShape(shapes.rectangle()),
                      foregroundStyle(
                        destructive ? Colors.destructive : Colors.ink,
                      ),
                    ]}
                  >
                    <Image
                      systemName={action.icon}
                      size={20}
                      modifiers={[frame({ width: 24 })]}
                    />
                    <Text
                      modifiers={[
                        font({
                          textStyle: "callout",
                          weight: "semibold",
                        }),
                      ]}
                    >
                      {action.label}
                    </Text>
                  </HStack>
                </Button>
              </VStack>
            );
          })}
        </VStack>
      </BottomSheet>
    </Host>
  );
}
