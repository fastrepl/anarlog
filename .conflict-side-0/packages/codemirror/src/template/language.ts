import { closePercentBrace, jinja } from "@codemirror/lang-jinja";

export function jinjaLanguage() {
  return [jinja(), closePercentBrace];
}
