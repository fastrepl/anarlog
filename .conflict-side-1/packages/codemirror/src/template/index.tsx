import CodeMirror from "@uiw/react-codemirror";

import { jinjaLanguage } from "./language";
import { templateExtensions } from "./theme";

export function TemplateEditor({
  value,
  onChange,
  placeholder,
  readOnly = false,
}: {
  value: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  readOnly?: boolean;
}) {
  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      readOnly={readOnly}
      basicSetup={{
        lineNumbers: false,
        foldGutter: false,
        highlightActiveLineGutter: false,
        highlightActiveLine: false,
      }}
      extensions={[jinjaLanguage(), ...templateExtensions]}
      height="100%"
    />
  );
}
