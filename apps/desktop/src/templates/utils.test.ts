import { describe, expect, it } from "vitest";

import type { WebTemplate } from "./codec";
import type { UserTemplate } from "./queries";
import { resolveTemplateTabSelection } from "./utils";

const userTemplate: UserTemplate = {
  id: "template-1",
  title: "Standup",
  description: "",
  pinned: false,
  sections: [],
};

const webTemplate: WebTemplate = {
  slug: "community-standup",
  title: "Community Standup",
  description: "",
  category: "",
  sections: [],
};

describe("resolveTemplateTabSelection", () => {
  it("keeps an empty tab in local template mode when there are no community templates", () => {
    expect(
      resolveTemplateTabSelection({
        isWebMode: true,
        selectedMineId: null,
        selectedWebIndex: null,
        userTemplates: [],
        webTemplates: [],
      }),
    ).toEqual({
      isWebMode: false,
      selectedMineId: null,
      selectedWebIndex: null,
      selectedWebTemplate: null,
    });
  });

  it("defaults to community mode only when community templates exist without local templates", () => {
    expect(
      resolveTemplateTabSelection({
        isWebMode: null,
        selectedMineId: null,
        selectedWebIndex: null,
        userTemplates: [],
        webTemplates: [webTemplate],
      }),
    ).toEqual({
      isWebMode: true,
      selectedMineId: null,
      selectedWebIndex: 0,
      selectedWebTemplate: webTemplate,
    });
  });

  it("selects the first local template when mine mode has no explicit selection", () => {
    expect(
      resolveTemplateTabSelection({
        isWebMode: false,
        selectedMineId: null,
        selectedWebIndex: null,
        userTemplates: [userTemplate],
        webTemplates: [webTemplate],
      }),
    ).toEqual({
      isWebMode: false,
      selectedMineId: "template-1",
      selectedWebIndex: null,
      selectedWebTemplate: null,
    });
  });
});
