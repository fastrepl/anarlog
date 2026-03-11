import { create } from "zustand";

import { type BasicActions, type BasicState, createBasicSlice } from "./basic";
import {
  type ChatModeActions,
  type ChatModeState,
  createChatModeSlice,
} from "./chat-mode";
import {
  createLifecycleSlice,
  type LifecycleActions,
  lifecycleMiddleware,
  type LifecycleState,
} from "./lifecycle";
import {
  createNavigationSlice,
  type NavigationActions,
  navigationMiddleware,
  type NavigationState,
} from "./navigation";
import {
  pinnedPersistenceMiddleware,
  restorePinnedTabsToStore,
} from "./pinned-persistence";
import {
  createRecentlyOpenedSlice,
  type RecentlyOpenedActions,
  recentlyOpenedMiddleware,
  type RecentlyOpenedState,
  restoreRecentlyOpenedToStore,
} from "./recently-opened";
import {
  createRestoreSlice,
  type RestoreActions,
  restoreMiddleware,
  type RestoreState,
} from "./restore";
import { createStateUpdaterSlice, type StateBasicActions } from "./state";

import { wrapSliceWithLogging } from "~/store/zustand/shared";

export type { ChatEvent, ChatMode } from "./chat-mode";
export type { SettingsState, SettingsTab, Tab, TabInput } from "./schema";
export { isSameTab, uniqueIdfromTab } from "./schema";
export { restorePinnedTabsToStore, restoreRecentlyOpenedToStore };

type State = BasicState &
  NavigationState &
  LifecycleState &
  RestoreState &
  RecentlyOpenedState &
  ChatModeState;
type Actions = BasicActions &
  StateBasicActions &
  NavigationActions &
  LifecycleActions &
  RestoreActions &
  RecentlyOpenedActions &
  ChatModeActions;
type Store = State & Actions;

export const useTabs = create<Store>()(
  recentlyOpenedMiddleware(
    pinnedPersistenceMiddleware(
      restoreMiddleware(
        lifecycleMiddleware(
          navigationMiddleware((set, get) => ({
            ...wrapSliceWithLogging("basic", createBasicSlice(set, get)),
            ...wrapSliceWithLogging("state", createStateUpdaterSlice(set, get)),
            ...wrapSliceWithLogging(
              "navigation",
              createNavigationSlice(set, get),
            ),
            ...wrapSliceWithLogging(
              "lifecycle",
              createLifecycleSlice(set, get),
            ),
            ...wrapSliceWithLogging("restore", createRestoreSlice(set, get)),
            ...wrapSliceWithLogging(
              "recentlyOpened",
              createRecentlyOpenedSlice(set, get),
            ),
            ...wrapSliceWithLogging("chatMode", createChatModeSlice(set, get)),
          })),
        ),
      ),
    ),
  ),
);
