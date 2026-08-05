---
name: product-update-newsletter
description: Write, update, or audit the Anarlog product-update newsletter campaign in Loops (app.loops.so) for a desktop release. Use after the changelog for the version is merged, when asked to draft, revise, or pre-send check the release announcement email.
metadata:
  internal: true
---

# Product Update Newsletter

Announce a desktop release to users through the "Anarlog update (...)" campaign in Loops.

Run this after `../new-changelog/SKILL.md` produces the entry and the version is released via
`../release-new-version/SKILL.md`. The published changelog is the copy source, not the raw commit log.

## Source of Truth

Read `https://anarlog.so/changelog/<version>`, which renders `packages/changelog/content/<version>.md`.

The changelog is frequently edited after first publish. When told it changed, re-fetch and diff
against the current email body, then apply only the delta. Do not reapply the whole rewrite.

Map items to the existing structure rather than appending a dump:

- Feature-level items become or extend an H3 section.
- Minor UX items become bullets under "Small things that add up".
- Reliability and privacy items append to the fixes paragraph after the bullets.

Keep beta or partially shipped work honestly qualified. If execution is not connected yet, say so.

## Campaign Setup

Duplicate the most recent "Anarlog update (...)" campaign from the Loops Home page so sender,
reply-to, section headings, Download button, and footer are inherited, then rename to the new date.

Campaign metrics pages show only send/open/click stats. Use the compose view to edit content.

## Body Structure Contract

Every section is exactly this block sequence, and edits must preserve it:

```text
H3 heading
P (empty spacer)
DIV.editor-image
P (empty spacer)
P (body text)
```

Verify before finishing:

```js
await page.evaluate(() => {
  const blocks = [...document.querySelector(".ContentEditable__root").children];
  return blocks
    .filter((b) => b.tagName === "H3")
    .map((h) => {
      const i = blocks.indexOf(h);
      return (
        h.innerText +
        " -> " +
        blocks
          .slice(i + 1, i + 4)
          .map((x) =>
            x.className.includes("editor-image")
              ? "IMG"
              : x.innerText.trim() === ""
                ? "SP"
                : "TXT",
          )
          .join(",")
      );
    });
});
```

## Editing the Lexical Body

The body is `div.ContentEditable__root`. The page has several `[contenteditable="true"]` elements
(Sender, From, Reply, Subject, Preview, body). Match on distinctive existing text, never a cached
index or ref. `###` creates an H3 and `-` creates a bullet.

Reliable mid-document replacement is Range-select then type:

```js
async function selectTextInBody(searchStr) {
  return await page.evaluate((str) => {
    const root = document.querySelector(".ContentEditable__root");
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let node;
    while ((node = walker.nextNode())) {
      const idx = node.textContent.indexOf(str);
      if (idx !== -1) {
        root.focus();
        const range = document.createRange();
        range.setStart(node, idx);
        range.setEnd(node, idx + str.length);
        const sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
        node.parentElement.scrollIntoView({ block: "center" });
        return true;
      }
    }
    return false;
  }, searchStr);
}
```

Repeat per occurrence for multi-hit edits such as version bumps.

### Failure Modes

These corrupt the document silently. All four were observed in practice.

- Range selection on the **first block** reverses typed characters, so "Hi," becomes ",iH". Lexical
  resets the caret to offset 0 per keystroke there. Use a native mouse triple-click, then type.
- `Meta+ArrowUp` does not reliably reach document start. It has appended to the end of the current
  block instead. Triple-click the target block.
- Coordinates read in the same `evaluate()` as `scrollIntoView` are stale, so clicks land in the
  wrong block and keystrokes corrupt unrelated sections. Scroll, `sleep(600)`, then re-read the rect
  in a second `evaluate()`.
- Pressing Enter with an active Range selection deletes the selected text. Press `End` or `Home` first.

Verify caret placement before typing:

```js
const ok = await page.evaluate(() => {
  const sel = window.getSelection();
  const blocks = [...document.querySelector(".ContentEditable__root").children];
  const target = blocks[TARGET_INDEX];
  const anchorBlock = sel.anchorNode
    ? (sel.anchorNode.nodeType === 1
        ? sel.anchorNode
        : sel.anchorNode.parentElement
      ).closest(".ContentEditable__root > *")
    : null;
  return anchorBlock === target;
});
```

If false, click the horizontal middle of the block's text rather than its left edge.

After any structural repair, scan all blocks for stray empty paragraphs, duplicated greetings, and
leftover `/` or `/image` text.

## Inserting an Image

The slash menu triggers a native file chooser. Arm the listener before opening the menu.

```js
const fcPromise = page
  .waitForEvent("filechooser", { timeout: 15000 })
  .catch(() => null);
await page.keyboard.type("/", { delay: 200 });
await sleep(700);
await page.keyboard.type("image", { delay: 120 });
await sleep(700);
const btn = await page.evaluate(() => {
  const menu = [...document.querySelectorAll("div.fixed")].find((el) =>
    el.className.toString().includes("z-[100]"),
  );
  const img =
    menu &&
    [...menu.querySelectorAll("button")].find(
      (b) => b.textContent.trim() === "Image",
    );
  const r = img.getBoundingClientRect();
  return { cx: r.x + r.width / 2, cy: r.y + r.height / 2 };
});
await page.mouse.click(btn.cx, btn.cy);
const fc = await fcPromise;
await fc.setFiles("/abs/path/to/image.png");
```

- The caret must sit in an empty paragraph, and `/` needs `delay >= 150` or a literal `/` is typed.
- The menu is a scrollable `div.fixed.z-[100]`. Unfiltered, the Image item under MEDIA sits below the
  visible scroll area, so clicking its reported rect hits the page behind and silently closes the
  menu. Typing `image` filters the list and brings it into view.
- `Enter` and `ArrowDown`+`Enter` do not activate the item. Only a mouse click works.
- Poll until `img.src` contains `images.vialoops.com` to confirm the upload finished.
- Insertion consumes the empty paragraph, so re-add the trailing spacer.

Replace an image by clicking it to select the block, pressing Backspace, then re-running the flow.
Capture existing `images.vialoops.com` URLs before editing so a deleted image can be re-downloaded
and restored.

## Section Art

Generate with Midjourney using the established house style:

```text
dithered. [short concept]. bright white sunshine, cheerful, high key. --ar 4:3 --profile aofpoq2
```

- Target bright, high-key, airy, light-background results. Reject dim or dystopian output.
- Prefer scene compositions, an object in a sunlit room, over flat grids or UI mockups. Grids do not
  match the existing set.
- The Personalize "P" toggle may fail to activate, leaving `data-active="false"`. Appending
  `--profile aofpoq2` to the prompt text works and produces the profile chip.
- Poll for completion by watching for `\d+% Complete` to disappear from `document.body.innerText`.

`cdn.midjourney.com` returns a Cloudflare challenge to Node-side `fetch()`. Download full resolution
inside the authenticated tab, where `jobId` comes from the grid thumbnail
`cdn.midjourney.com/<jobId>/0_<n>_640_N.webp`:

```js
const b64 = await mjPage.evaluate(async (u) => {
  const r = await fetch(u);
  const blob = await r.blob();
  return await new Promise((res) => {
    const fr = new FileReader();
    fr.onload = () => res(fr.result.split(",")[1]);
    fr.readAsDataURL(blob);
  });
}, `https://cdn.midjourney.com/${jobId}/0_${variant}.png`);
```

## Pre-Send Audit

Run before sending and report pass or fail per item.

Automated:

- Version string consistent across subject and body, with no prior version anywhere.
- Every changelog item is represented in the copy.
- All H3 sections match the `SP,IMG,SP` pattern.
- All images report `complete && naturalWidth > 0`.
- No stray artifacts: `/image`, orphan `/`, double spaces, duplicated greeting.
- Sign-off block intact.

Manual judgment:

- Sender, From, and Reply fields correct. Flag any reply-to change, since the body promises a
  personal reply and any call for testers routes to the same address.
- Preview text length. Clients truncate near 90 to 140 characters, so front-load the important content.
- The Download button URL lives in editor state, not the DOM. Click the button block and read the
  Link textbox in the right sidebar.
- State explicitly that only the Compose step was audited unless Audience, Schedule, and Goals were
  also checked.

## Reporting

Restate what changed in plain language, quote the new copy, and attach a screenshot of edited
sections. Disclose any self-inflicted document damage and its repair rather than hiding it.
