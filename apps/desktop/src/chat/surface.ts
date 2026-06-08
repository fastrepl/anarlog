export type ChatToolbarSurface = "light" | "dark";

// Chat chrome always uses the original stone-800 shell, independent of app theme.
export function isChatDarkAppearance(): boolean {
  return true;
}

export function chatPanelClassNames(): string {
  return "bg-primary text-primary-foreground";
}

export function chatPanelBorderClassNames(): string {
  return "border-stone-700/80";
}

export function chatFloatingPanelShellClassNames(): string {
  return "bg-primary text-primary-foreground rounded-2xl border-2 border-stone-600 shadow-[0_16px_48px_rgba(0,0,0,0.55)]";
}

export function chatElevatedSurfaceClassNames(): string {
  return "bg-primary-foreground text-primary border-border";
}

export function chatInputEditorClassNames(): string {
  return "chat-input-editor text-primary";
}

export function chatSendButtonDisabledClassNames(): string {
  return "cursor-default border-border text-muted-foreground/60";
}

export function chatSendButtonShortcutDisabledClassNames(): string {
  return "text-muted-foreground/60";
}

export function chatToolbarSurface(): ChatToolbarSurface {
  return "dark";
}

export function chatFloatingControlClassNames(): string {
  return "border-border bg-primary-foreground text-primary hover:bg-primary-foreground/90";
}
