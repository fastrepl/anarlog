import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

export type TodoProvider = {
  id: string;
  displayName: string;
  icon: ReactNode;
  nangoIntegrationId?: string;
  filterLabel?: string;
  filterPlaceholder?: string;
  permission?: "reminders";
  platform?: "macos" | "all";
};

const styles = stylex.create({
  icon: {
    height: "1.25rem",
    width: "1.25rem",
  },
  remindersIcon: {
    borderRadius: "4px",
    height: "1.25rem",
    objectFit: "cover",
    width: "1.25rem",
  },
});

export const TODO_PROVIDERS: TodoProvider[] = [
  {
    id: "github",
    displayName: "GitHub",
    icon: (
      <img
        src="/assets/github-icon.svg"
        alt="GitHub"
        {...stylex.props(styles.icon)}
      />
    ),
    nangoIntegrationId: "github",
    filterLabel: "Repository",
    filterPlaceholder: "e.g. owner/repo",
    platform: "all",
  },
  {
    id: "apple-reminders",
    displayName: "Apple Reminders",
    icon: (
      <img
        src="/assets/apple-reminders.png"
        alt="Apple Reminders"
        {...stylex.props(styles.remindersIcon)}
      />
    ),
    permission: "reminders",
    platform: "macos",
  },
];
