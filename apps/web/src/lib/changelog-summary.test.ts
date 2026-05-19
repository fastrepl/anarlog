import assert from "node:assert/strict";
import test from "node:test";

import { getEntrySummary } from "./changelog-summary.ts";

test("uses the first sentence from explicit summaries", () => {
  assert.equal(
    getEntrySummary("Recording improved. Follow-up details stay hidden."),
    "Recording improved.",
  );
});

test("skips custom tags and strips markdown from fallback content", () => {
  const content = `
<banner title="Local STT models are back!" variant="info">
- **Parakeet V3** and Whisper Small should work smoothly.
- Local STT processing is now batch only.
</banner>
`;

  assert.equal(
    getEntrySummary(content),
    "Parakeet V3 and Whisper Small should work smoothly.",
  );
});

test("ignores headings and images when building fallback content", () => {
  const content = `
# Changelog
![hero](/api/assets/example.png)
## Transcript
- \`Cmd/Ctrl+W\` now closes the desktop window when no tab is open
`;

  assert.equal(
    getEntrySummary(content),
    "Cmd/Ctrl+W now closes the desktop window when no tab is open",
  );
});
