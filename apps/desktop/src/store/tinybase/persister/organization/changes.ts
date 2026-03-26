import { createMarkdownEntityParser } from "~/store/tinybase/persister/shared/paths";

export const parseOrganizationIdFromPath =
  createMarkdownEntityParser("organizations");
