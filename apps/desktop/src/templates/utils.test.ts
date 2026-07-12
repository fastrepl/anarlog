import { describe, expect, it } from "vitest";

import type { WebTemplate } from "./codec";
import type { UserTemplate } from "./queries";
import { DEFAULT_TEMPLATE_ICON } from "./template-icon";
import {
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

describe("filterWebTemplatesAgainstUserTemplates", () => {
  it("drops web templates that already exist locally by title", () => {
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
  });

  it("normalizes punctuation when matching template titles", () => {
    const duplicateWebTemplate = {
      ...webTemplate,
      slug: "one-on-one-meeting",
      title: "1:1 Meeting",
    };

    expect(
      filterWebTemplatesAgainstUserTemplates({
        userTemplates: [{ ...userTemplate, title: "1 1 meeting" }],
        webTemplates: [duplicateWebTemplate],
      }),
    ).toEqual([]);
  });
});
