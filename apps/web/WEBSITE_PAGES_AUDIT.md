# Website Pages Audit

## List 1: All Pages by Hierarchy Level

### Level 1 -- Main Navigation (Header Menu)

These pages are directly accessible from the header navigation on every page.

| URL | Page Name |
|-----|-----------|
| `/` | Home |
| `/why-hyprnote/` | Why Hyprnote |
| `/pricing/` | Pricing |
| `/blog/` | Blog (index) |
| `/changelog/` | Changelog (index) |
| `/roadmap/` | Roadmap (index) |
| `/company-handbook/` | Company Handbook (redirects to first page) |
| `/opensource/` | Open Source |
| `/enterprise/` | Enterprise |
| `/product/ai-notetaking` | AI Notetaking |
| `/product/search` | Searchable Notes |
| `/product/markdown` | Markdown Files |
| `/product/flexible-ai` | Flexible AI |
| `/product/api` | API |
| `/solution/knowledge-workers` | For Knowledge Workers |
| `/gallery/templates` | Gallery - Templates view |

### Level 1 -- Footer Navigation

These pages appear in the footer on every page (some overlap with header).

| URL | Page Name | Footer Column |
|-----|-----------|---------------|
| `/download/` | Download | Product |
| `/docs/` | Docs (redirects to first page) | Product |
| `/about/` | About Us | Company |
| `/jobs/` | Jobs | Company |
| `/brand/` | Brand | Company |
| `/press-kit/` | Press Kit | Company |
| `/gallery/` | Prompt Gallery | Resources |
| `/docs/faq` | FAQ (redirects to first FAQ page) | Resources |
| `/eval/` | AI Eval | Tools |
| `/file-transcription/` | Audio Transcription | Tools |
| `/oss-friends/` | OSS Navigator | Tools |
| `/legal/terms` | Terms | Brand section |
| `/legal/privacy` | Privacy | Brand section |
| `/auth/` | Get Started (sign up) | Brand section |
| `/vs/{random}` | Random comparison page | Resources (random) |
| `/solution/{random}` | Random solution page | Resources (random) |

### Level 2 -- Linked from Specific Pages

These pages are NOT in the header/footer but are linked from the body of other pages.

| URL | Linked From |
|-----|-------------|
| `/founders/` | `/pricing`, `/enterprise`, `/download` |
| `/product/extensions` | `/product/ai-assistant` |
| `/product/self-hosting` | `/free`, `/product/local-ai` |
| `/product/ai-assistant` | `/` (home) |
| `/product/ai-notetaking/` | `/` (home), `/solution/*` pages, `/product/notepad` |
| `/templates/` | `/` (home), `/gallery`, `/vs/*`, `/integrations/*` |
| `/shortcuts/` | `/gallery` |
| `/privacy/` | `/security` |
| `/press-kit/app/` | `/press-kit` |
| `/free/` | *(needs verification -- may be linked from pricing or other)* |
| `/security/` | *(needs verification)* |
| `/download/apple-silicon` | `<DownloadButton>` component (platform-dependent) |
| `/download/windows` | `<DownloadButton>` component (platform-dependent) |
| `/download/linux-deb` | *(from download index page)* |
| `/download/linux-appimage` | *(from download index page)* |
| `/download/apple-intel` | *(from download index page)* |

### Level 2 -- Content-Driven Pages (from list/index pages)

These are individual content pages accessible from their parent list page.

| URL Pattern | Parent Page | Count |
|-------------|-------------|-------|
| `/blog/{slug}` | `/blog` | 58 articles |
| `/changelog/{version}` | `/changelog` | 68 versions |
| `/docs/{section}/{page}` | `/docs` sidebar | ~45 pages |
| `/company-handbook/{section}/{page}` | `/company-handbook` sidebar | ~56 pages |
| `/vs/{slug}` | Footer (random), blog cross-links | 23 comparisons |
| `/integrations/{category}/{slug}` | *(no index page found)* | 12 pages |
| `/templates/{slug}` | `/templates`, `/gallery` | 17 templates |
| `/shortcuts/{slug}` | `/shortcuts`, `/gallery` | 6 shortcuts |
| `/roadmap/{slug}` | `/roadmap` | 12 items |
| `/legal/{slug}` | `/legal` | 4 documents |
| `/jobs/{slug}` | `/jobs` | 2 listings |
| `/gallery/{type}/{slug}` | `/gallery` | *(dynamic, from templates+shortcuts)* |
| `/k6-reports/{id}` | `/k6-reports` | *(dynamic, internal)* |

### Level 3 -- Pages with No Incoming Links (Orphan Pages)

These pages exist as routes but are NOT linked from the header, footer, or any other page body.

| URL | Description |
|-----|-------------|
| `/product/bot` | Coming Soon page -- not linked from anywhere |
| `/product/memory` | Coming Soon page -- not linked from anywhere |
| `/product/notepad` | Has links TO other pages, but no pages link TO it |
| `/product/integrations` | Not linked from nav or other pages |
| `/product/local-ai` | Not linked from nav or other pages |
| `/product/mini-apps` | Not linked from nav or other pages |
| `/product/ai-assistant` | Only linked from home page (not nav) |
| `/solution/coaching` | Only reachable from footer random link |
| `/solution/consulting` | Only reachable from footer random link |
| `/solution/customer-success` | Not linked from anywhere |
| `/solution/engineering` | Not linked from anywhere |
| `/solution/field-engineering` | Not linked from anywhere |
| `/solution/government` | Not linked from anywhere |
| `/solution/healthcare` | Not linked from anywhere |
| `/solution/journalism` | Only reachable from footer random link |
| `/solution/legal` | Not linked from anywhere |
| `/solution/media` | Not linked from anywhere |
| `/solution/meeting` | Not linked from anywhere |
| `/solution/project-management` | Not linked from anywhere |
| `/solution/recruiting` | Only reachable from footer random link |
| `/solution/research` | Only reachable from footer random link |
| `/solution/sales` | Only reachable from footer random link |
| `/bounties/` | Redirect to GitHub -- no incoming links |
| `/contact/` | Redirect to mailto -- no incoming links |
| `/k6-reports/` | Internal tool -- no incoming links |

### Utility / Redirect Pages (not real content pages)

| URL | Purpose |
|-----|---------|
| `/bluesky` | Redirect to Bluesky profile |
| `/discord` | Redirect to Discord server |
| `/github` | Redirect to GitHub repo |
| `/linkedin` | Redirect to LinkedIn page |
| `/reddit` | Redirect to Reddit |
| `/x` | Redirect to Twitter/X |
| `/youtube` | Redirect to YouTube |
| `/bounties` | Redirect to GitHub |
| `/contact` | Redirect to email |
| `/founders` | Redirect to cal.com |
| `/callback/auth` | Auth callback handler |
| `/callback/signout` | Sign-out callback handler |
| `/reset-password` | Password reset form |
| `/update-password` | Password update form |

### Authenticated Pages (require login)

| URL | Purpose |
|-----|---------|
| `/app` | User dashboard |
| `/app/account` | Account settings |
| `/app/checkout` | Checkout flow |
| `/app/file-transcription` | File transcription (authenticated) |
| `/app/integration` | Integration management |

### Admin Pages (require admin auth)

| URL | Purpose |
|-----|---------|
| `/admin` | Admin dashboard |
| `/admin/collections` | Content management |
| `/admin/media` | Media management |
| `/admin/stars` | GitHub stars tracking |
| `/admin/crm` | CRM |
| `/admin/lead-finder` | Lead finder |
| `/admin/kanban` | Kanban board |

---

## List 2: Pages with Very Little or No Content

### No Content / Pure Redirects

These pages render nothing -- they immediately redirect.

| URL | What it does |
|-----|-------------|
| `/docs/` | Redirects to `/docs/about/hello-world` (11 lines) |
| `/company-handbook/` | Redirects to first handbook page (11 lines) |
| `/download/apple-silicon` | Redirects to external download URL (10 lines) |
| `/download/apple-intel` | Redirects to external download URL (10 lines) |
| `/download/windows` | Redirects to external download URL (11 lines) |
| `/download/linux-deb` | Redirects to external download URL (10 lines) |
| `/download/linux-appimage` | Redirects to external download URL (10 lines) |
| `/bounties` | Redirects to GitHub (external) |
| `/contact` | Redirects to mailto link |
| `/founders` | Redirects to cal.com |

### Minimal Content (1 screen or a couple sentences)

These pages have very little actual content -- typically just a title, one sentence, and a button.

| URL | Lines | What's there |
|-----|-------|-------------|
| `/product/memory` | 53 | Title + "Coming Soon" badge + 1 sentence |
| `/product/flexible-ai` | 54 | Title + 1 sentence + "Download" button |
| `/product/markdown` | 54 | Title + 1 sentence + "Download" button |
| `/product/extensions` | 81 | "Coming Soon" + list of extension tags |
| `/product/api` | 93 | "Coming Soon" + mock terminal animation |
| `/legal/` (index) | 99 | Simple list of legal document links |
| `/k6-reports/` | 100 | Internal tool -- data table |
| `/product/integrations` | 124 | Hero section + grid of integration logos |
| `/jobs/` | 145 | Job listing cards (depends on MDX content -- currently 2 jobs) |
| `/product/bot` | 187 | "Coming Soon" + draggable meeting bot icons |
| `/product/search` | 200 | Hero + 1 feature section with mock search UI |
| `/changelog/` | 206 | List page -- content depends on MDX entries |

### Product Pages Summary

| URL | Content Level | Notes |
|-----|--------------|-------|
| `/product/ai-notetaking` | Full (2476 lines) | Interactive demos, multiple sections |
| `/product/mini-apps` | Full (638 lines) | Multiple sections with examples |
| `/product/self-hosting` | Full (524 lines) | Feature sections, comparisons |
| `/product/local-ai` | Full (479 lines) | Multiple feature sections |
| `/product/ai-assistant` | Full (465 lines) | Feature sections with examples |
| `/product/notepad` | Moderate (278 lines) | Mock window demo + features |
| `/product/search` | Moderate (200 lines) | Hero + mock search UI |
| `/product/bot` | Minimal (187 lines) | Coming Soon placeholder |
| `/product/integrations` | Minimal (124 lines) | Logo grid, no real content |
| `/product/api` | Minimal (93 lines) | Coming Soon placeholder |
| `/product/extensions` | Minimal (81 lines) | Coming Soon placeholder |
| `/product/flexible-ai` | Minimal (54 lines) | 1 sentence + button |
| `/product/markdown` | Minimal (54 lines) | 1 sentence + button |
| `/product/memory` | Minimal (53 lines) | 1 sentence + Coming Soon |

### Solution Pages Summary

All solution pages have `noindex, nofollow` meta tags.

| URL | Content Level | Notes |
|-----|--------------|-------|
| `/solution/engineering` | Full (574 lines) | Unique layout with multiple sections |
| `/solution/meeting` | Moderate (378 lines) | Unique layout |
| `/solution/coaching` | Template (246 lines) | Hero + 6 cards + table + CTA |
| `/solution/consulting` | Template (246 lines) | Same template |
| `/solution/customer-success` | Template (~246 lines) | Same template |
| `/solution/field-engineering` | Template (~246 lines) | Same template |
| `/solution/government` | Template (~246 lines) | Same template |
| `/solution/healthcare` | Template (~246 lines) | Same template |
| `/solution/journalism` | Template (~246 lines) | Same template |
| `/solution/knowledge-workers` | Template (~246 lines) | Same template |
| `/solution/legal` | Template (~246 lines) | Same template |
| `/solution/media` | Template (~246 lines) | Same template |
| `/solution/project-management` | Template (~246 lines) | Same template |
| `/solution/recruiting` | Template (~246 lines) | Same template |
| `/solution/research` | Template (~246 lines) | Same template |
| `/solution/sales` | Template (~246 lines) | Same template |

> Note: The 13 "template" solution pages all follow an identical structure with industry-specific copy swapped in. They have content, but it's formulaic (hero, 6 feature cards, comparison table, use cases, CTA).

### Notes

- All product and solution pages have `noindex, nofollow` robots meta tags
- Several handbook MDX files have duplicate slugs which could cause build issues
- The `/integrations/*` pages have no index/list page -- they're only reachable via direct URL or search
- `/oss-friends` renders 85 entries from MDX but has no individual detail pages

---

## SEO & Performance Suggestions

Based on the full codebase analysis (TanStack Start on Vite 7, deployed to Netlify, content via content-collections/MDX).

### Critical Priority (High SEO Impact)

#### 1. Fix the Domain Migration Leftovers (`hyprnote.com` → `char.com`)

Several places in the code still reference the old domain `hyprnote.com`. Search engines see these as signals about where the "real" site lives, so they need to point to the current domain.

| File | What's Wrong |
|------|-------------|
| `src/routes/__root.tsx` line 34 | `ai-sitemap` meta tag points to `https://hyprnote.com/llms.txt` |
| `src/routes/__root.tsx` line 39 | `og:url` is `https://hyprnote.com` |
| `src/routes/__root.tsx` line 51 | `twitter:url` is `https://hyprnote.com` |
| `src/routes/_view/blog/$slug.tsx` line 54 | Blog canonical URLs use `https://hyprnote.com/blog/...` |
| `src/routes/_view/blog/$slug.tsx` line 60 | Blog OG image fallback URL uses `https://hyprnote.com/og?...` |
| `public/llms.txt` | References `hyprnote.com` throughout |

**What to do:** Find-and-replace `https://hyprnote.com` → `https://char.com` in all source files listed above. The Netlify 301 redirects handle visitors, but meta tags and canonical URLs should point to the canonical domain directly.

#### 2. Add Canonical Tags to All Public Pages

A "canonical tag" tells Google "this is the one true URL for this content" — it prevents duplicate-content issues (e.g. if someone links to your page with `?utm_source=...` query parameters, Google still knows which URL to rank).

Currently only `/blog/$slug` pages have canonical tags. Every indexable public page should have one.

**What to do:** Add a `<link rel="canonical" href="https://char.com/current-path" />` to the `head()` of every public route file. Consider creating a shared helper:

```typescript
function canonicalUrl(path: string) {
  return { tag: "link", attrs: { rel: "canonical", href: `https://char.com${path}` } };
}
```

#### 3. Re-evaluate the Aggressive `noindex` Strategy

Right now **44+ pages** are marked `noindex, nofollow` — meaning Google is told to completely ignore them. This includes pages that could bring organic traffic:

| Pages | Status | Recommendation |
|-------|--------|----------------|
| `/product/ai-notetaking` (2476 lines, rich content) | noindex | **Should be indexed** — this is a flagship feature page |
| `/product/self-hosting` (524 lines) | noindex | **Should be indexed** — self-hosting is a differentiator |
| `/product/local-ai` (479 lines) | noindex | **Should be indexed** — privacy/local AI is a key selling point |
| `/product/ai-assistant` (465 lines) | noindex | **Should be indexed** |
| `/product/mini-apps` (638 lines) | noindex | **Should be indexed** |
| `/vs/*` (23 comparison pages) | noindex | **Should be indexed** — comparison pages are high-intent SEO gold |
| `/solution/engineering`, `/solution/meeting` | noindex | Consider indexing the ones with unique content |
| `/product/bot`, `/product/memory`, `/product/api`, `/product/extensions` | noindex | OK to keep noindex — these are "Coming Soon" stubs |
| Template solution pages (13 pages, same layout) | noindex | OK to keep noindex until content is differentiated |

**What to do:** Remove `noindex, nofollow` from content-rich product pages and all `/vs/*` pages. Update `robots.txt` accordingly (remove the `Disallow: /product/` and `Disallow: /vs/` lines, or make them more specific). Add these routes to the sitemap.

**Contradiction to fix:** `/solution/*` and `/vs/*` are currently prerendered (SSG) but also noindexed and blocked in robots.txt. You're spending build time generating HTML that Google is told to ignore.

#### 4. Add Structured Data (JSON-LD / Schema.org)

Structured data is invisible markup that helps Google understand *what* your content is (a product, an article, an FAQ, etc.). It can unlock "rich results" in search — like star ratings, FAQ dropdowns, breadcrumbs, and article cards.

Currently: **no structured data anywhere on the site**.

**What to add:**

| Schema Type | Where | Why |
|-------------|-------|-----|
| `Organization` | Root layout (`__root.tsx`) | Tells Google about Char as a company (name, logo, social links) |
| `WebSite` with `SearchAction` | Homepage | Enables sitelinks search box in Google |
| `Article` | `/blog/$slug` pages | Rich article cards in search results (author, date, image) |
| `BreadcrumbList` | `/docs/*`, `/blog/*`, `/company-handbook/*` | Breadcrumb trail in search results |
| `SoftwareApplication` | `/download` or homepage | Product info for software (name, OS, price: "Free") |
| `FAQPage` | `/docs/faq` pages | FAQ dropdowns directly in search results |
| `Product` | `/pricing` | Product name, offers, pricing tiers |

**Example for the root layout:**

```json
{
  "@context": "https://schema.org",
  "@type": "Organization",
  "name": "Char",
  "url": "https://char.com",
  "logo": "https://char.com/api/images/hyprnote/og-image.jpg",
  "sameAs": [
    "https://github.com/nichochar/hyprnote",
    "https://x.com/getcharnotes",
    "https://linkedin.com/company/char"
  ]
}
```

---

### High Priority (Performance Impact)

#### 5. Fix Font Loading (Currently Render-Blocking)

Fonts are one of the biggest performance issues found. Two problems:

**Problem A:** Google Fonts loaded via CSS `@import` in `styles.css` line 1. The `@import` method is "render-blocking" — the browser must download the CSS file from Google's server before it can show any text. This delays the First Contentful Paint (FCP).

**What to do:** Replace the `@import` with `<link rel="preconnect">` and `<link rel="stylesheet">` tags in the `<head>` of `__root.tsx`. This lets the browser start fetching fonts earlier, in parallel with other resources:

```html
<link rel="preconnect" href="https://fonts.googleapis.com" />
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Lora:wght@400;500;600;700&display=swap&family=Instrument+Serif:ital@1&display=swap" />
```

**Problem B:** The 7 self-hosted `@font-face` declarations (Redaction, SF Pro) in `styles.css` are missing `font-display: swap`. Without it, browsers may show invisible text while fonts load.

**What to do:** Add `font-display: swap;` to every `@font-face` block. This tells the browser: "show fallback text immediately, then swap in the custom font when it loads."

#### 6. Add Netlify Custom Headers (Security + Caching)

The `netlify.toml` has no `[[headers]]` blocks. This means:
- No security headers (browsers don't know your security preferences)
- No cache-control hints for static assets (fonts, images, JS get default short caching)

**What to add to `netlify.toml`:**

```toml
[[headers]]
  for = "/*"
  [headers.values]
    X-Frame-Options = "SAMEORIGIN"
    X-Content-Type-Options = "nosniff"
    Referrer-Policy = "strict-origin-when-cross-origin"
    Permissions-Policy = "camera=(), microphone=(), geolocation=()"

[[headers]]
  for = "/fonts/*"
  [headers.values]
    Cache-Control = "public, max-age=31536000, immutable"

[[headers]]
  for = "/icons/*"
  [headers.values]
    Cache-Control = "public, max-age=31536000, immutable"

[[headers]]
  for = "/*.js"
  [headers.values]
    Cache-Control = "public, max-age=31536000, immutable"

[[headers]]
  for = "/*.css"
  [headers.values]
    Cache-Control = "public, max-age=31536000, immutable"
```

The `immutable` cache directive tells the browser: "this file will never change at this URL, so don't bother re-checking." Vite already puts content hashes in filenames, so this is safe.

#### 7. Fix the `manifest.json` (Still Default Boilerplate)

The web app manifest still says `"name": "Create TanStack App Sample"` — this is the default template value. While this file mainly affects PWA behavior and "Add to Home Screen," Google also reads it.

**What to do:**

```json
{
  "short_name": "Char",
  "name": "Char - AI Notepad",
  "icons": [
    { "src": "favicon.ico", "sizes": "64x64 32x32 24x24 16x16", "type": "image/x-icon" }
  ],
  "start_url": "/",
  "display": "standalone",
  "theme_color": "#000000",
  "background_color": "#ffffff"
}
```

Also: the manifest references `logo192.png` and `logo512.png` which don't exist in `public/`. Either add them or remove the references to avoid 404 errors.

---

### Medium Priority (SEO Improvements)

#### 8. Expand the Sitemap

The sitemap currently includes ~18 static routes and dynamic blog/docs/changelog/gallery content — but it's missing several indexable pages:

| Missing from Sitemap | Should Be Added |
|----------------------|----------------|
| `/why-hyprnote` | Yes |
| `/jobs/` and `/jobs/$slug` | Yes |
| `/templates/` and `/templates/$slug` | Yes |
| `/shortcuts/` and `/shortcuts/$slug` | Yes |
| `/product/*` (content-rich ones) | Yes, once noindex is removed |
| `/vs/$slug` | Yes, once noindex is removed |
| `/integrations/$category/$slug` | Yes, once noindex is removed |

#### 9. Create Missing Index/List Pages

Several content types have individual pages but no list/index page to browse them:

| Content Type | Has Index? | Suggestion |
|--------------|-----------|------------|
| `/integrations/*` | No | Create `/integrations` index page — lists all 12 integration pages |
| `/templates/*` | Redirects to `/gallery` | Consider a dedicated `/templates` list page for SEO |
| `/shortcuts/*` | Redirects to `/gallery` | Consider a dedicated `/shortcuts` list page |
| `/vs/*` | No | Create `/vs` index page — "How Char compares to alternatives" |

Index pages serve as "hub" pages for SEO — they link to all child pages in one place, which helps Google discover and rank them.

#### 10. Add `og:url` Per-Page (Not Just Global)

The global `og:url` in `__root.tsx` is set to the homepage for every page. When someone shares `/pricing` on social media, the OpenGraph URL still says `https://char.com` (actually still `https://hyprnote.com`). Each page should set its own `og:url` to its actual URL.

#### 11. Expand Prerendering

Currently only `/`, `/blog/*`, `/docs/*`, `/pricing`, `/solution/*`, and `/vs/*` are prerendered (turned into static HTML at build time). Other content-heavy pages like `/changelog/*`, `/gallery/*`, `/enterprise`, `/about`, `/download` are server-rendered on each request.

Prerendering makes pages load faster (no server wait time) and is better for SEO (Google gets instant HTML). Consider adding to the prerender filter in `vite.config.ts`:

```typescript
filter: ({ path }) => {
  return (
    path === "/" ||
    path.startsWith("/blog") ||
    path.startsWith("/docs") ||
    path.startsWith("/pricing") ||
    path.startsWith("/solution") ||
    path.startsWith("/vs") ||
    path.startsWith("/changelog") ||
    path.startsWith("/gallery") ||
    path.startsWith("/about") ||
    path.startsWith("/enterprise") ||
    path.startsWith("/download") ||
    path.startsWith("/product") ||
    path.startsWith("/why-hyprnote")
  );
},
```

---

### Lower Priority (Nice to Have)

#### 12. Measure Core Web Vitals

The `web-vitals` package is installed as a devDependency but not used anywhere in the codebase. Core Web Vitals (LCP, FID, CLS) are a Google ranking factor.

**What to do:** Either integrate `web-vitals` to report to PostHog/Sentry, or use Netlify Analytics (which includes Web Vitals automatically). This gives real user data about how fast the site feels.

#### 13. Add Favicon Variants and Apple Touch Icon

Currently only `favicon.ico` exists. Modern browsers and devices expect:

| Asset | Purpose |
|-------|---------|
| `apple-touch-icon.png` (180×180) | iOS home screen icon |
| `favicon-32x32.png` | Modern browsers tab icon |
| `favicon-16x16.png` | Smaller contexts |

Add them to `public/` and reference them in `__root.tsx` head links.

#### 14. Convert `.otf` Fonts to `.woff2`

The self-hosted fonts (Redaction, SF Pro) are in `.otf` format. The `.woff2` format is ~30% smaller and specifically designed for web use. All modern browsers support it.

**What to do:** Convert the 7 `.otf` files in `public/fonts/` to `.woff2` (using a tool like `fonttools` or an online converter), then update the `@font-face` `src` URLs in `styles.css`.

#### 15. Fix Orphan Pages or Remove Them

From the audit above, 25+ pages have no incoming links. Search engines can still find them via sitemap, but pages with no internal links get very little "link equity" (ranking power).

**Options for each orphan:**
- **If the page is useful:** Add links to it from relevant pages (e.g. link `/product/integrations` from the footer or a product overview page)
- **If the page is a stub/placeholder:** Keep `noindex` and consider removing from the build entirely until real content exists
- **If the page is a duplicate:** Redirect it to the canonical version

#### 16. Add `hreflang` If Internationalization Is Planned

Currently no language variants exist — the site is English-only with `<html lang="en">`. If there are plans for other languages, `hreflang` tags will be needed. No action required right now, but worth noting for future planning.

#### 17. Reduce Third-Party Script Impact

Two external scripts load on every public page:
- **Zendesk chat widget** (`ze-snippet`) — these are notoriously heavy (~200-400KB)
- **PostHog analytics** — relatively light but still adds to load time

**What to do:** Consider lazy-loading Zendesk (load it only after user interaction or after a delay) instead of loading it immediately on page load. This can improve Time to Interactive significantly.
