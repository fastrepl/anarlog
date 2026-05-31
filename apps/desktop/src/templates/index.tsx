import { TemplateView } from "./template-body";

import { StandardTabWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

export { parseWebTemplates } from "./codec";
export type { WebTemplate } from "./codec";
export {
  useCreateTemplate,
  useUserTemplate,
  useUserTemplates,
} from "./queries";
export type { UserTemplate, UserTemplateDraft } from "./queries";
export {
  filterWebTemplatesAgainstUserTemplates,
  getTemplateCreatorLabel,
  useTemplateCreatorName,
} from "./utils";
export { TemplatesSidebarContent } from "./template-sidebar";

export function TabContentTemplate({
  tab,
}: {
  tab: Extract<Tab, { type: "templates" }>;
}) {
  return (
    <StandardTabWrapper>
      <TemplateView tab={tab} />
    </StandardTabWrapper>
  );
}
