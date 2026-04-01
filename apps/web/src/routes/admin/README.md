# Admin Interface

The admin interface lives under `/admin` in the web app and is intended for internal content and growth workflows.

`/admin/` redirects to `/admin/collections/`. The shared layout is defined in `src/routes/admin/route.tsx` and marks the entire area `noindex, nofollow`.

## Authentication

Admin access is gated by `ADMIN_EMAILS` in `src/lib/team.ts`.

- In development, the admin route returns a mock `dev@local` admin user and the admin API routes skip auth checks.
- In production, unauthenticated users are redirected to `/auth/?flow=web&provider=github&redirect=...&rra=true`.
- Authenticated non-admin users are redirected to `/`.
- Client-side admin fetches should go through `fetchAdminJson()` in `src/lib/admin-auth.ts`; it redirects back to sign-in on `401` responses or expired GitHub credentials.

The content editor has an additional GitHub credential requirement. Admin users need a row in the Supabase `admins` table with `github_token` and `github_username`. If those credentials are missing or invalid, the UI sends the user through GitHub reauth.

## Routes

### Content (`/admin/collections/`)

The content editor manages three collections:

- `articles` from `apps/web/content/articles`
- `docs` from `apps/web/content/docs`
- `handbook` from `apps/web/content/handbook`

Current capabilities include:

- browse articles, docs, handbook pages, and draft branches in one editor
- edit MDX content with TipTap
- edit collection-specific metadata
- preview rendered output next to the editor
- create, rename, duplicate, delete, and publish content
- upload blog images and import article content from Google Docs
- track pending PRs and branch-backed drafts

Save behavior depends on whether the file is already on a branch:

- saving a published file creates or reuses a `blog/...` branch and opens or updates a PR
- saving a draft branch writes directly to that branch
- articles, docs, and handbook entries all use the same branch and PR flow for published edits

### Media (`/admin/media/`)

The media library manages files in Supabase Storage.

Current capabilities include:

- browse folders and files with tabbed navigation
- upload files and register them with the admin library
- create folders
- move, rename, download, and delete assets
- filter, search, multi-select, and drag items between folders

### CRM (`/admin/crm/`)

The CRM route is currently a client-side contact scratchpad. Contacts, filters, and edits live in local React state only. There is no server persistence or admin API backing this screen yet.

### Lead Finder (`/admin/lead-finder/`)

Lead Finder is the main GitHub lead-qualification workflow.

It is backed by `public.github_star_leads` and supports:

- paginated lead browsing
- search and researched-only filtering
- fetching new leads from GitHub stargazers or org activity
- researching individual leads with OpenRouter
- bulk research for the top unresearched leads on the current page

### Kanban (`/admin/kanban/`)

Kanban is a lightweight GitHub Projects v2 client for `fastrepl/marketing`.

It supports:

- listing available projects
- viewing project items by status
- creating GitHub issues and adding them to the active project
- updating issue fields and project status
- deleting project items and closing issues

### Stars (`/admin/stars/`)

`/admin/stars/` is still a live route, but it is effectively an older internal view over the same star-lead APIs used by Lead Finder. It is not linked from the current admin header.

## API Endpoints

All `/api/admin/**` routes enforce admin auth outside development mode.

### Content APIs

- `GET /api/admin/content/list`
- `GET /api/admin/content/list-drafts`
- `GET /api/admin/content/get-branch-file`
- `GET /api/admin/content/history`
- `GET /api/admin/content/pending-pr`
- `POST /api/admin/content/create`
- `POST /api/admin/content/save`
- `POST /api/admin/content/publish`
- `POST /api/admin/content/rename`
- `POST /api/admin/content/duplicate`
- `POST /api/admin/content/delete`

### Media APIs

- `GET /api/admin/media/list`
- `GET /api/admin/media/download`
- `POST /api/admin/media/upload`
- `POST /api/admin/media/register`
- `POST /api/admin/media/delete`
- `POST /api/admin/media/move`
- `POST /api/admin/media/create-folder`

### Import APIs

- `POST /api/admin/blog/upload-image`
- `POST /api/admin/import/google-docs`

### Kanban APIs

- `GET /api/admin/kanban/projects`
- `GET /api/admin/kanban/items`
- `POST /api/admin/kanban/create`
- `POST /api/admin/kanban/update`
- `POST /api/admin/kanban/delete`

### Lead APIs

- `GET /api/admin/stars/leads`
- `POST /api/admin/stars/fetch`
- `POST /api/admin/stars/research`

## Configuration

The admin surface depends on a few different backends, and the required configuration varies by feature.

### Required for admin auth and media

- `SUPABASE_URL`
- `SUPABASE_ANON_KEY`
- `SUPABASE_SERVICE_ROLE_KEY` for server-side privileged storage operations
- `VITE_SUPABASE_URL`
- `VITE_SUPABASE_ANON_KEY`

### Required for content editing

- valid GitHub credentials stored per admin user in the Supabase `admins` table

### Required for Kanban and GitHub lead ingestion

- `GITHUB_TOKEN`

### Required for lead storage and research

- `DATABASE_URL`
- `OPENROUTER_API_KEY`, unless the caller passes `apiKey` directly to `/api/admin/stars/research`

## Code Map

- `src/routes/admin/` page routes and shared admin layout
- `src/routes/api/admin/` server endpoints
- `src/functions/admin.ts` admin user and GitHub credential helpers
- `src/lib/admin-auth.ts` client-side auth and reauth helpers
- `src/functions/github-content.ts` content save, branch, and PR logic
- `src/functions/github-projects.ts` GitHub Projects integration for Kanban
- `src/functions/github-stars.ts` GitHub lead ingestion and research
- `src/hooks/use-media-api.tsx` media client helpers
