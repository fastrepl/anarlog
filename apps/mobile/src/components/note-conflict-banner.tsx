import { useMutation } from "@tanstack/react-query";
import { Text as NativeText, View } from "react-native";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Spacing, Typography } from "@/constants/theme";
import {
  resolveSessionConflicts,
  restoreConflict,
  useSessionConflicts,
  type RestoredNote,
  type SessionConflict,
} from "@/data/conflicts";
import { Text } from "@/i18n";
import { createStyleHook } from "@/settings/theme-provider";

export function NoteConflictBanner({
  sessionId,
  onBeforeRestore,
  onRestored,
}: {
  sessionId: string;
  onBeforeRestore: () => void | Promise<void>;
  onRestored: (restored: RestoredNote) => void;
}) {
  const conflicts = useSessionConflicts(sessionId);
  const newest = conflicts.data[0];
  if (!newest) return null;
  return (
    <ConflictBanner
      key={newest.id}
      conflict={newest}
      sessionId={sessionId}
      onBeforeRestore={onBeforeRestore}
      onRestored={onRestored}
    />
  );
}

function ConflictBanner({
  conflict,
  sessionId,
  onBeforeRestore,
  onRestored,
}: {
  conflict: SessionConflict;
  sessionId: string;
  onBeforeRestore: () => void | Promise<void>;
  onRestored: (restored: RestoredNote) => void;
}) {
  const styles = useStyles();
  const keep = useMutation({
    mutationFn: () => resolveSessionConflicts(sessionId),
  });
  const restore = useMutation({
    mutationFn: async () => {
      await onBeforeRestore();
      return restoreConflict(conflict);
    },
    onSuccess: onRestored,
  });
  const busy = keep.isPending || restore.isPending;

  return (
    <Card style={styles.card} tone="muted">
      <Text style={styles.copy}>
        Edited on another device at the same time. The later edit was kept.
      </Text>
      {(keep.error || restore.error) && (
        <Text accessibilityRole="alert" style={styles.error}>
          Couldn’t update this note. Try again.
        </Text>
      )}
      <View style={styles.actions}>
        <Button
          label="Keep"
          disabled={busy}
          onPress={() => keep.mutate()}
          size="small"
          variant="outline"
        />
        <Button
          label="Use other version"
          disabled={busy}
          loading={restore.isPending}
          onPress={() => restore.mutate()}
          size="small"
          variant="ghost"
        />
      </View>
    </Card>
  );
}

const useStyles = createStyleHook((Colors) => ({
  card: {
    marginHorizontal: Spacing.md,
    marginTop: Spacing.sm,
    padding: Spacing.md,
  },
  copy: {
    ...Typography.caption,
    color: Colors.ink,
  },
  error: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.destructive,
  },
  actions: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    marginTop: Spacing.sm,
  },
}));
