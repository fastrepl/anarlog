# @hypr/tiptap

Opinionated Tiptap wrapper with preconfigured extensions, styles, and editor components.

## What's in the box

### Editor presets

| Export | Description |
|--------|-------------|
| `@hypr/tiptap/editor` | Document editor with mentions, file handling, keyboard navigation, and debounced updates. |
| `@hypr/tiptap/chat` | Chat editor with Enter-to-submit, slash commands, and mentions. |
| `@hypr/tiptap/prompt` | Jinja-aware prompt template editor built on CodeMirror. |

### Shared extensions (`@hypr/tiptap/shared`)

All presets pull from the same extension bundle:

- StarterKit (bold, italic, strike, code, headings, lists, blockquote, code block, horizontal rule, hard break)
- Tables (resizable), task lists (nestable), images, links, YouTube embeds
- Hashtag highlighting, AI content highlights, search & replace, streaming animation
- Markdown conversion (`json2md` / `md2json`), content validation, clipboard serialization

### Styles (`@hypr/tiptap/styles.css`)

```css
@import "@hypr/tiptap/styles.css";
```

One import, styles every node type.

## Quick start

### Document editor

```tsx
import Editor from "@hypr/tiptap/editor";
import "@hypr/tiptap/styles.css";

function NotePage() {
  return (
    <Editor
      initialContent={doc}
      editable={true}
      handleChange={(content) => save(content)}
      placeholderComponent={({ node }) =>
        node.type.name === "paragraph" ? "Start writing..." : ""
      }
    />
  );
}
```

### Chat editor

```tsx
import ChatEditor from "@hypr/tiptap/chat";
import "@hypr/tiptap/styles.css";

function ChatInput() {
  const ref = useRef(null);

  return (
    <ChatEditor
      ref={ref}
      editable={true}
      onSubmit={() => send(ref.current?.editor?.getJSON())}
      slashCommandConfig={{
        handleSearch: (query) => searchCommands(query),
      }}
    />
  );
}
```

### Prompt template editor

```tsx
import { PromptEditor } from "@hypr/tiptap/prompt";

function TemplateEditor() {
  return (
    <PromptEditor
      value={template}
      onChange={setTemplate}
      variables={["name", "context"]}
      filters={["upper", "truncate"]}
      placeholder="Write your prompt template..."
    />
  );
}
```

## Utilities

```ts
import {
  json2md,
  md2json,
  isValidTiptapContent,
  parseJsonContent,
  extractHashtags,
  EMPTY_TIPTAP_DOC,
} from "@hypr/tiptap/shared";

const markdown = json2md(tiptapJson);
const json = md2json("# Hello\n\nWorld");

if (isValidTiptapContent(data)) {
  editor.commands.setContent(data);
}

const tags = extractHashtags(htmlString);
```

## License

[MIT](./LICENSE)
