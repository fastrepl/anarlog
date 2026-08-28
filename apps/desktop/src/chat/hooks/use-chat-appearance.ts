import {
  chatElevatedSurfaceStyle,
  chatInputEditorStyle,
  chatPanelBorderStyle,
  chatPanelStyle,
  chatSendButtonDisabledStyle,
  chatSendButtonShortcutDisabledStyle,
  chatToolbarSurface,
  isChatDarkAppearance,
} from "~/chat/surface";

export function useChatAppearance() {
  const isDarkAppearance = isChatDarkAppearance();

  return {
    isDarkAppearance,
    toolbarSurface: chatToolbarSurface(),
    panelStyle: chatPanelStyle,
    panelBorderStyle: chatPanelBorderStyle,
    elevatedSurfaceStyle: chatElevatedSurfaceStyle,
    inputEditorStyle: chatInputEditorStyle,
    sendButtonDisabledStyle: chatSendButtonDisabledStyle,
    sendButtonShortcutDisabledStyle: chatSendButtonShortcutDisabledStyle,
  };
}
