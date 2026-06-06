import {
  chatElevatedSurfaceClassNames,
  chatInputEditorClassNames,
  chatPanelBorderClassNames,
  chatPanelClassNames,
  chatSendButtonDisabledClassNames,
  chatSendButtonShortcutDisabledClassNames,
  chatToolbarSurface,
  isChatDarkAppearance,
} from "~/chat/surface";

export function useChatAppearance() {
  const isDarkAppearance = isChatDarkAppearance();

  return {
    isDarkAppearance,
    toolbarSurface: chatToolbarSurface(),
    panelClassName: chatPanelClassNames(),
    panelBorderClassName: chatPanelBorderClassNames(),
    elevatedSurfaceClassName: chatElevatedSurfaceClassNames(),
    inputEditorClassName: chatInputEditorClassNames(),
    sendButtonDisabledClassName: chatSendButtonDisabledClassNames(),
    sendButtonShortcutDisabledClassName:
      chatSendButtonShortcutDisabledClassNames(),
  };
}
