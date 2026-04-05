import { TabContentCalendar } from "~/calendar";
import { TabContentChangelog } from "~/changelog";
import { TabContentChat } from "~/chat/tab/tab-content";
import { TabContentChatShortcut } from "~/chat_shortcuts";
import { TabContentContact } from "~/contacts";
import { TabContentHuman } from "~/contacts/humans";
import { TabContentDaily } from "~/daily";
import { TabContentEdit } from "~/edit";
import { TabContentFolder } from "~/folders";
import { TabContentOnboarding } from "~/onboarding";
import { TabContentNote } from "~/session";
import { TabContentSettings } from "~/settings";
import { TabContentEmpty } from "~/shared/main/empty";
import { type Tab } from "~/store/zustand/tabs";
import { TabContentTemplate } from "~/templates";

export function MainTabContent({ tab }: { tab: Tab }) {
  if (tab.type === "sessions") {
    return <TabContentNote tab={tab} />;
  }
  if (tab.type === "folders") {
    return <TabContentFolder tab={tab} />;
  }
  if (tab.type === "humans") {
    return <TabContentHuman tab={tab} />;
  }
  if (tab.type === "contacts") {
    return <TabContentContact tab={tab} />;
  }

  if (tab.type === "empty") {
    return <TabContentEmpty tab={tab} />;
  }
  if (tab.type === "calendar") {
    return <TabContentCalendar />;
  }
  if (tab.type === "changelog") {
    return <TabContentChangelog tab={tab} />;
  }
  if (tab.type === "settings") {
    return <TabContentSettings tab={tab} />;
  }
  if (tab.type === "templates") {
    return <TabContentTemplate tab={tab} />;
  }
  if (tab.type === "chat_shortcuts") {
    return <TabContentChatShortcut tab={tab} />;
  }
  if (tab.type === "chat_support") {
    return <TabContentChat tab={tab} />;
  }
  if (tab.type === "onboarding") {
    return <TabContentOnboarding tab={tab} />;
  }
  if (tab.type === "daily") {
    return <TabContentDaily />;
  }
  if (tab.type === "edit") {
    return <TabContentEdit tab={tab} />;
  }
  return null;
}
