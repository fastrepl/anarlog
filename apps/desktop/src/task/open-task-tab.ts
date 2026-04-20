import type { Node } from "prosemirror-model";

import type { AppLinkAttrs } from "@hypr/editor/app-link";

import { id } from "~/shared/utils";
import { type Tab, type TaskResource, useTabs } from "~/store/zustand/tabs";

export function collectSiblingResources(
  doc: Node,
  pos: number,
): TaskResource[] {
  const $pos = doc.resolve(pos);

  let listItem: Node | null = null;
  for (let depth = $pos.depth; depth >= 0; depth--) {
    const node = $pos.node(depth);
    if (node.type.name === "listItem" || node.type.name === "taskItem") {
      listItem = node;
      break;
    }
  }

  const resources: TaskResource[] = [];
  const target = listItem ?? doc;

  target.descendants((node) => {
    if (node.type.name !== "appLink") return true;
    const attrs = node.attrs as AppLinkAttrs;
    if (attrs.provider !== "github") return false;
    if (!attrs.owner || !attrs.repo || !attrs.number) return false;

    if (attrs.kind === "issue") {
      resources.push({
        type: "github_issue",
        owner: attrs.owner,
        repo: attrs.repo,
        number: attrs.number,
      });
    } else if (attrs.kind === "pull_request") {
      resources.push({
        type: "github_pr",
        owner: attrs.owner,
        repo: attrs.repo,
        number: attrs.number,
      });
    }

    return false;
  });

  return resources;
}

export function openTaskTab(resources: TaskResource[]) {
  const state = useTabs.getState();
  const { tabs } = state;

  const newTab: Tab = {
    type: "task",
    id: id(),
    resources,
    active: true,
    slotId: id(),
    pinned: false,
  };

  const existing = tabs.find(
    (tab) =>
      tab.type === "task" &&
      tab.resources.length === newTab.resources.length &&
      tab.resources.every((resource, index) => {
        const next = newTab.resources[index];
        return (
          resource.type === next.type &&
          resource.owner === next.owner &&
          resource.repo === next.repo &&
          resource.number === next.number
        );
      }),
  );
  if (existing) {
    state.select(existing);
    return;
  }

  const deactivated = tabs.map((tab) => ({ ...tab, active: false }));
  useTabs.setState({
    tabs: [...deactivated, newTab],
    currentTab: newTab,
  });
}
