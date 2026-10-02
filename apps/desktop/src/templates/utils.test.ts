import { describe, expect, it } from "vitest";

import type { WebTemplate } from "./codec";
import type { UserTemplate } from "./queries";
import { DEFAULT_TEMPLATE_ICON } from "./template-icon";
import {
  AUTO_TEMPLATE_ID,
  filterWebTemplatesAgainstUserTemplates,
  resolveTemplateTabSelection,
} from "./utils";

const userTemplate: UserTemplate = {
  id: "template-1",
  title: "Standup",
  description: "",
  pinned: false,
  icon: DEFAULT_TEMPLATE_ICON,
  sections: [],
};

const webTemplate: WebTemplate = {
  slug: "community-standup",
  title: "Community Standup",
  description: "",
  category: "",
  icon: DEFAULT_TEMPLATE_ICON,
  sections: [],
};

describe("resolveTemplateTabSelection", () => {
  it.each([
    [
      "keeps an empty tab in local template mode when there are no community templates",
      {
        isWebMode: true,
        selectedMineId: null,
        selectedWebIndex: null,
        userTemplates: [],
        webTemplates: [],
      },
      {
        isWebMode: false,
        selectedMineId: AUTO_TEMPLATE_ID,
        selectedWebIndex: null,
        selectedWebTemplate: null,
      },
    ],
    [
      "defaults to community mode only when community templates exist without local templates",
      {
        isWebMode: null,
        selectedMineId: null,
        selectedWebIndex: null,
        userTemplates: [],
        webTemplates: [webTemplate],
      },
      {
        isWebMode: true,
        selectedMineId: null,
        selectedWebIndex: 0,
        selectedWebTemplate: webTemplate,
      },
    ],
    [
      "selects the first local template when mine mode has no explicit selection",
      {
        isWebMode: false,
        selectedMineId: null,
        selectedWebIndex: null,
        userTemplates: [userTemplate],
        webTemplates: [webTemplate],
      },
      {
        isWebMode: false,
        selectedMineId: "template-1",
        selectedWebIndex: null,
        selectedWebTemplate: null,
      },
    ],
    [
      "preserves an explicit Auto selection when local templates exist",
      {
        isWebMode: false,
        selectedMineId: AUTO_TEMPLATE_ID,
        selectedWebIndex: null,
        userTemplates: [userTemplate],
        webTemplates: [webTemplate],
      },
      {
        isWebMode: false,
        selectedMineId: AUTO_TEMPLATE_ID,
        selectedWebIndex: null,
        selectedWebTemplate: null,
      },
    ],
  ])("%s", (_name, input, expected) => {
    expect(resolveTemplateTabSelection(input)).toEqual(expected);
  });
});

describe("filterWebTemplatesAgainstUserTemplates", () => {
  it("drops web templates that already exist locally by normalized title", () => {
    const duplicateWebTemplate = {
      ...webTemplate,
      slug: "daily-standup",
      title: "Daily Standup",
    };
    const uniqueWebTemplate = {
      ...webTemplate,
      slug: "sales-discovery-call",
      title: "Sales Discovery Call",
    };

    expect(
      filterWebTemplatesAgainstUserTemplates({
        userTemplates: [{ ...userTemplate, title: "daily standup" }],
        webTemplates: [duplicateWebTemplate, uniqueWebTemplate],
      }),
    ).toEqual([uniqueWebTemplate]);

    expect(
      filterWebTemplatesAgainstUserTemplates({
        userTemplates: [{ ...userTemplate, title: "1 1 meeting" }],
        webTemplates: [
          { ...webTemplate, slug: "one-on-one-meeting", title: "1:1 Meeting" },
        ],
      }),
    ).toEqual([]);
  });
});
