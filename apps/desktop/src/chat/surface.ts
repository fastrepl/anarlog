export type ChatToolbarSurface = "light" | "dark";

export function isChatDarkAppearance(): boolean {
  return false;
}

export function chatPanelClassNames(): string {
  return "bg-card text-card-foreground";
}

export function chatFloatingPanelClassNames(): string {
  return "bg-[#f4f4f5] text-card-foreground dark:bg-[#202020]";
}

export function chatPanelBorderClassNames(): string {
  return "border-border";
}

export function chatFloatingPanelShellClassNames(): string {
  return "bg-[#f4f4f5] text-card-foreground rounded-2xl border-2 border-[#dedede] shadow-[0_16px_48px_rgba(0,0,0,0.18)] dark:border-[#3a3a3a] dark:bg-[#202020] dark:shadow-[0_16px_48px_rgba(0,0,0,0.55)]";
}

export function chatElevatedSurfaceClassNames(): string {
  return "bg-card text-card-foreground border-border";
}

export function chatInputEditorClassNames(): string {
  return "chat-input-editor text-card-foreground";
}

export function chatSendButtonDisabledClassNames(): string {
  return "cursor-default border-border text-muted-foreground/60";
}

export function chatSendButtonShortcutDisabledClassNames(): string {
  return "text-muted-foreground/60";
}

export function chatToolbarSurface(): ChatToolbarSurface {
  return "light";
}

export function chatFloatingControlClassNames(): string {
  return "border-border bg-accent text-accent-foreground hover:bg-accent/90";
}
