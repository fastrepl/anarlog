import { useRef, useState } from "react";
import {
  InputAccessoryView,
  Keyboard,
  Platform,
  TextInput,
  View,
} from "react-native";

import { EditorAccessory } from "@/components/editor-accessory";
import { Spacing, Typography } from "@/constants/theme";
import { insertCapturedNoteAttachmentMarkdown } from "@/data/note-attachment-model";
import { applyEditorFormat, type EditorFormat } from "@/lib/editor-format";
import { useMountEffect } from "@/lib/use-mount-effect";
import { createStyleHook, useColors } from "@/settings/theme-provider";

export function BodyEditor({
  accessoryId,
  defaultBodyFormat,
  defaultValue,
  editable,
  onAttach,
  onChangeText,
  onCommit,
  onFocusChange,
}: {
  accessoryId: string;
  defaultBodyFormat: "prosemirror_json" | "markdown";
  defaultValue: string;
  editable: boolean;
  onAttach: (signal: AbortSignal) => Promise<{ markdown: string } | null>;
  onChangeText: (
    body: string,
    bodyFormat: "prosemirror_json" | "markdown",
  ) => void;
  onCommit: () => void;
  onFocusChange: (focused: boolean) => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const inputRef = useRef<TextInput>(null);
  const textRef = useRef(defaultValue);
  const bodyFormatRef = useRef(defaultBodyFormat);
  const selectionRef = useRef({ start: 0, end: 0 });
  // Normal typing stays native so iOS retains its caret and scroll state;
  // toolbar commands briefly override both without remounting the editor.
  const [nativeOverride, setNativeOverride] = useState<{
    text: string;
    selection: { start: number; end: number };
  }>();
  const [androidKeyboardVisible, setAndroidKeyboardVisible] = useState(false);
  const [attaching, setAttaching] = useState(false);
  const attachControllerRef = useRef<AbortController | null>(null);
  const editorActiveRef = useRef(true);

  useMountEffect(() => {
    editorActiveRef.current = true;
    return () => {
      editorActiveRef.current = false;
      attachControllerRef.current?.abort();
    };
  });

  useMountEffect(() => {
    if (Platform.OS !== "android") return;
    const showSubscription = Keyboard.addListener("keyboardDidShow", () =>
      setAndroidKeyboardVisible(true),
    );
    const hideSubscription = Keyboard.addListener("keyboardDidHide", () =>
      setAndroidKeyboardVisible(false),
    );
    return () => {
      showSubscription.remove();
      hideSubscription.remove();
    };
  });

  const handleChangeText = (body: string) => {
    textRef.current = body;
    onChangeText(body, bodyFormatRef.current);
  };

  const handleFormat = (format: EditorFormat) => {
    if (attaching) return;
    const formatted = applyEditorFormat(
      textRef.current,
      selectionRef.current,
      format,
    );
    textRef.current = formatted.text;
    bodyFormatRef.current = formatted.bodyFormat;
    selectionRef.current = formatted.selection;
    setNativeOverride({
      text: formatted.text,
      selection: formatted.selection,
    });
    onChangeText(formatted.text, formatted.bodyFormat);
    inputRef.current?.focus();
    requestAnimationFrame(() => setNativeOverride(undefined));
  };

  const handleDismissKeyboard = () => {
    inputRef.current?.blur();
    Keyboard.dismiss();
  };

  const handleAttach = async () => {
    if (attachControllerRef.current) return;
    const controller = new AbortController();
    const capturedText = textRef.current;
    const capturedSelection = { ...selectionRef.current };
    attachControllerRef.current = controller;
    setAttaching(true);
    try {
      const attachment = await onAttach(controller.signal);
      if (
        !attachment ||
        controller.signal.aborted ||
        !editorActiveRef.current
      ) {
        return;
      }
      const inserted = insertCapturedNoteAttachmentMarkdown({
        capturedText,
        capturedSelection,
        currentText: textRef.current,
        markdown: attachment.markdown,
      });
      textRef.current = inserted.text;
      bodyFormatRef.current = "markdown";
      selectionRef.current = inserted.selection;
      setNativeOverride(inserted);
      onChangeText(inserted.text, "markdown");
      onCommit();
      inputRef.current?.focus();
      requestAnimationFrame(() => {
        if (editorActiveRef.current) setNativeOverride(undefined);
      });
    } finally {
      if (attachControllerRef.current === controller) {
        attachControllerRef.current = null;
      }
      if (editorActiveRef.current) setAttaching(false);
    }
  };

  return (
    <>
      <TextInput
        ref={inputRef}
        style={styles.body}
        multiline
        editable={editable && !attaching}
        inputAccessoryViewID={Platform.OS === "ios" ? accessoryId : undefined}
        defaultValue={defaultValue}
        value={nativeOverride?.text}
        selection={nativeOverride?.selection}
        placeholder="Start typing…"
        placeholderTextColor={Colors.muted}
        textAlignVertical="top"
        onChangeText={handleChangeText}
        onBlur={() => onFocusChange(false)}
        onFocus={() => onFocusChange(true)}
        onSelectionChange={(event) => {
          selectionRef.current = event.nativeEvent.selection;
        }}
      />
      {Platform.OS === "ios" && editable && (
        <InputAccessoryView
          nativeID={accessoryId}
          backgroundColor={Colors.background}
        >
          <EditorAccessory
            attaching={attaching}
            onAttach={() => void handleAttach()}
            onFormat={handleFormat}
            onDismiss={handleDismissKeyboard}
          />
        </InputAccessoryView>
      )}
      {Platform.OS === "android" && editable && androidKeyboardVisible && (
        <View style={styles.androidAccessory}>
          <EditorAccessory
            attaching={attaching}
            onAttach={() => void handleAttach()}
            onFormat={handleFormat}
            onDismiss={handleDismissKeyboard}
          />
        </View>
      )}
    </>
  );
}

const useStyles = createStyleHook((Colors) => ({
  body: {
    flex: 1,
    paddingHorizontal: Spacing.md,
    paddingTop: Spacing.md,
    ...Typography.body,
    color: Colors.ink,
  },
  androidAccessory: { backgroundColor: Colors.background },
}));
