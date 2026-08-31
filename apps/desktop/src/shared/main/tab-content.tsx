import { lazy, Suspense } from "react";

import { type Tab } from "~/store/zustand/tabs";

const TabContentAutomations = lazy(async () => ({
  default: (await import("~/settings/automations")).TabContentAutomations,
}));
const TabContentFolders = lazy(async () => ({
  default: (await import("~/folders")).TabContentFolders,
}));
const TabContentCalendar = lazy(async () => ({
  default: (await import("~/calendar")).TabContentCalendar,
}));
const TabContentChangelog = lazy(async () => ({
  default: (await import("~/changelog")).TabContentChangelog,
}));
const TabContentContact = lazy(async () => ({
  default: (await import("~/contacts")).TabContentContact,
}));
const TabContentHuman = lazy(async () => ({
  default: (await import("~/contacts/humans")).TabContentHuman,
}));
const TabContentEdit = lazy(async () => ({
  default: (await import("~/edit")).TabContentEdit,
}));
const TabContentNote = lazy(async () => ({
  default: (await import("~/session")).TabContentNote,
}));
const TabContentOnboarding = lazy(async () => ({
  default: (await import("~/onboarding")).TabContentOnboarding,
}));
const TabContentSettings = lazy(async () => ({
  default: (await import("~/settings")).TabContentSettings,
}));
const TabContentSharedNote = lazy(async () => ({
  default: (await import("~/shared-notes")).TabContentSharedNote,
}));
const TabContentSharedNotePreview = lazy(async () => ({
  default: (await import("~/shared-notes")).TabContentSharedNotePreview,
}));
const TabContentTask = lazy(async () => ({
  default: (await import("~/task")).TabContentTask,
}));
const TabContentTemplate = lazy(async () => ({
  default: (await import("~/templates")).TabContentTemplate,
}));

export function MainTabContent({ tab }: { tab: Tab }) {
  return (
    <Suspense fallback={null}>
      <LazyTabContent tab={tab} />
    </Suspense>
  );
}

function LazyTabContent({ tab }: { tab: Tab }) {
  if (tab.type === "automations") {
    return <TabContentAutomations />;
  }
  if (tab.type === "folders") {
    return <TabContentFolders />;
  }
  if (tab.type === "sessions") {
    return <TabContentNote tab={tab} />;
  }
  if (tab.type === "shared_sessions") {
    return <TabContentSharedNote tab={tab} />;
  }
  if (tab.type === "shared_note_preview") {
    return <TabContentSharedNotePreview tab={tab} />;
  }
  if (tab.type === "humans") {
    return <TabContentHuman tab={tab} />;
  }
  if (tab.type === "contacts") {
    return <TabContentContact tab={tab} />;
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
  if (tab.type === "onboarding") {
    return <TabContentOnboarding tab={tab} />;
  }
  if (tab.type === "edit") {
    return <TabContentEdit tab={tab} />;
  }
  if (tab.type === "task") {
    return <TabContentTask tab={tab} />;
  }
  return null;
}
