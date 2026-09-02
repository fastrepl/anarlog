export type QuickAction = "toggle_listening";

export type AnarlogQuickActionsModuleEvents = {
  onAction: (event: { action: QuickAction }) => void;
};
