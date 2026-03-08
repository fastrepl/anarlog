# Admin Interface

The admin interface at `/admin` provides content management capabilities for the Char website.

## Authentication

Access is restricted to whitelisted email addresses defined in `src/lib/team.ts` (`ADMIN_EMAILS`).

Users must be authenticated via Supabase to access admin routes. Non-admin users are redirected to the home page. In development mode, authentication is bypassed with a mock `dev@local` user.

## Features

### Media Library (`/admin/media`)

Upload, organize, and manage media assets stored in Supabase Storage.

- Drag-and-drop file upload with per-file progress toasts
- Folder navigation with lazy-loaded folder tree in sidebar
- Tab-based navigation with pinning, reordering, and back/forward history
- Multi-select with batch delete and download
- Context menu for individual file actions (rename, replace, copy path, download, move, delete)
- Sidebar with search, file type filters, and expandable folder tree
- Drag-and-drop files between folders
- Resizable sidebar/content panels

### Content Management (`/admin/collections`)

Full-featured blog editor with the following capabilities:

- Create, edit, and manage blog articles
- Rich text editor (TipTap) with Google Docs import modal
- Metadata panel (title, display title, description, author, date, category, cover image, featured flag)
- Preview mode with side-by-side editing via resizable panels
- Git history tracking per file
- Draft management with branch-based workflow
- Tab-based file navigation with pinning, reordering, and close actions
- Inline new post creation and file renaming in sidebar
- Media selector modal for choosing cover images from the media library
- Clipboard operations (cut, copy) for content items
- Auto-save countdown indicator

Requires GitHub credentials (stored in Supabase `admins` table). Users without GitHub credentials are redirected to authenticate via GitHub.

#### Editorial Workflow

Complete flow from editing to publication:

**1. User Edits a Published Article**
- Open `/admin/collections` and select a published article
- Make changes in the editor
- Auto-save runs every 60 seconds, or save manually with ⌘S / Save button

**2. Save Creates a PR**
- Creates a new branch `blog/{slug}-{timestamp}` (or uses existing one)
- Creates a non-draft PR to `main`, ready to merge
- Assigns `harshikaalagh-netizen` as reviewer on PR creation
- A banner appears in the editor linking to the PR

**3. GitHub Actions Trigger**
- `blog-auto-format.yml` - Auto-formats MDX files with dprint, commits changes back to branch
- `blog-grammar-check.yml` - Runs AI grammar check (Anthropic), posts suggestions as PR comment

**4. User Continues Editing (Optional)**
- Each save updates the same PR branch
- Each push triggers the grammar check again

**5. Reviewer Merges PR**
- Article goes live on the website

## API Endpoints

All API endpoints require admin authentication (bypassed in development mode).

### Media APIs

- `GET /api/admin/media/list` - List files in a directory
- `POST /api/admin/media/upload` - Upload files (base64 content)
- `POST /api/admin/media/delete` - Delete files (batch)
- `POST /api/admin/media/move` - Move/rename files
- `POST /api/admin/media/create-folder` - Create folders

### Blog APIs

- `POST /api/admin/blog/upload-image` - Upload images for blog posts to Supabase Storage

### Import APIs

- `POST /api/admin/import/google-docs` - Parse published Google Doc
- `POST /api/admin/import/save` - Save MDX file to repository

### Content APIs

- `GET /api/admin/content/list` - List content files in a folder
- `GET /api/admin/content/list-drafts` - List draft articles from branches
- `GET /api/admin/content/pending-pr` - Check if article has a pending edit PR
- `GET /api/admin/content/get-branch-file` - Get file content from a branch
- `GET /api/admin/content/history` - Get git commit history for a file
- `POST /api/admin/content/save` - Save content (creates PR for published articles)
- `POST /api/admin/content/create` - Create new content file
- `POST /api/admin/content/publish` - Publish/unpublish an article
- `POST /api/admin/content/rename` - Rename a content file
- `POST /api/admin/content/duplicate` - Duplicate a content file
- `POST /api/admin/content/delete` - Delete a content file

## GitHub Workflows

The editorial workflow is powered by three GitHub Actions workflows in `.github/workflows/`:

- **`blog-auto-format.yml`** - Auto-formats MDX files with dprint on push to `blog/**` branches and commits changes back
- **`blog-grammar-check.yml`** - Runs AI-powered grammar check (Anthropic) on article PRs and posts suggestions as comments
- **`blog-slack-notify.yml`** - Sends Slack notifications for article changes with editorial status detection (edit, new article, submit for review, unpublish)

`blog-grammar-check.yml` and `blog-slack-notify.yml` trigger on PRs to `main` that modify `apps/web/content/articles/**` on `blog/` branches. `blog-auto-format.yml` triggers on pushes to `blog/**` branches.

## Environment Variables

The following environment variables are required:

- `GITHUB_TOKEN` - GitHub personal access token with repo write access
- `ANTHROPIC_API_KEY` - Anthropic API key for AI grammar checking
- `SLACK_BOT_TOKEN` - Slack bot token for sending notifications
- `SLACK_BLOG_CHANNEL_ID` - Slack channel ID for blog notifications
- Supabase environment variables for authentication and storage

## Development

The admin interface uses TanStack Router (React Start) with file-based routing. Routes are defined in:

- `src/routes/admin/` - Page components
- `src/routes/api/admin/` - API endpoints
- `src/hooks/use-media-api.tsx` - Media API client hook
- `src/functions/admin.ts` - Admin authentication helpers
- `src/lib/team.ts` - Admin email whitelist and team member data

Admin authentication is handled by the `fetchAdminUser()` function which checks if the current user's email is in the `ADMIN_EMAILS` whitelist.
