# remarkable-cli

`remarkable-cli` syncs notes from a reMarkable tablet to a local folder by connecting to the reMarkable cloud, downloading documents, and writing a readable local copy of each document plus its attachments.

## Install

Requires Node.js `>=18`.

Install globally:

```bash
npm install -g remarkable-cli
```

Run without installing globally:

```bash
npx remarkable-cli@latest --help
```

If installed globally, the command is:

```bash
remarkable-cli
```

## Quick Start

From zero to first sync:

```bash
npx remarkable-cli@latest init
npx remarkable-cli@latest sync
npx remarkable-cli@latest status
```

What happens:

1. `init` opens the reMarkable device registration page, asks for the one-time code, and saves your local config.
2. `sync` downloads documents from the reMarkable cloud into your sync folder.
3. `status` shows whether the device is connected, where files are being synced, and when the last sync ran.

## Commands Reference

### `init`

Connects this machine to your reMarkable account.

Usage:

```bash
remarkable-cli init
```

What it does:

1. Opens `https://my.remarkable.com/device/desktop/connect` in your browser when possible.
2. Prompts for the one-time code from reMarkable.
3. Registers this machine as a device and verifies the returned token.
4. Prompts for a sync folder. Default: `~/remarkable`.
5. Writes auth and config files to `~/.config/remarkable-cli/`.

Flags:

```text
None
```

Example:

```bash
remarkable-cli init
```

### `sync`

Syncs documents from the reMarkable cloud to the local sync folder.

Usage:

```bash
remarkable-cli sync [options]
```

Flags:

```text
--force    Re-download all documents by clearing local sync state before syncing
--dry-run  Declared by the CLI, but currently not implemented in the sync logic
```

Examples:

Normal sync:

```bash
remarkable-cli sync
```

Force a full re-download:

```bash
remarkable-cli sync --force
```

Pass the declared dry-run flag:

```bash
remarkable-cli sync --dry-run
```

Notes:

- `sync` requires a prior `init`.
- If you are not connected, the command exits with an error and tells you to run `remarkable-cli init`.
- `--force` resets in-memory sync state for that run, so every document is treated as needing a fresh sync.
- `--dry-run` is accepted by the CLI parser, but the current implementation does not change behavior. It still performs a normal sync.

During sync, the CLI:

1. Refreshes the reMarkable user token from the stored device token.
2. Lists cloud items from the root tree.
3. Reuses cached metadata where possible.
4. Downloads changed documents.
5. Writes local Markdown files and attachment files.
6. Updates local sync state and a sync log.

### `status`

Shows connection status and the current local sync state.

Usage:

```bash
remarkable-cli status
```

Flags:

```text
None
```

What it prints:

- Whether this machine is connected to reMarkable
- The expanded sync folder path
- Last sync time
- Number of synced documents and folders tracked in local state
- Summary of the last sync report
- Whether PDF syncing is excluded

Example:

```bash
remarkable-cli status
```

If the device is not connected, `status` stops after showing that and tells you to run `remarkable-cli init`.

### `reset`

Clears local sync state. The next sync will treat documents as new and re-download them.

Usage:

```bash
remarkable-cli reset [options]
```

Flags:

```text
--auth   Also clear stored device authentication and disconnect this machine
```

Examples:

Reset sync state only:

```bash
remarkable-cli reset
```

Reset sync state and disconnect the device:

```bash
remarkable-cli reset --auth
```

What changes:

- Always resets `state.json` to an empty sync state.
- With `--auth`, also clears the stored device token and device ID in `auth.json`.

## Default Behavior With No Command

Running `remarkable-cli` without a subcommand uses the stored auth state:

- If the device is not connected, it runs the same flow as `init`.
- If the device is connected, it runs the same flow as `sync`.

Examples:

```bash
remarkable-cli
npx remarkable-cli@latest
```

## Config

All local data is stored under:

```text
~/.config/remarkable-cli/
```

Files used by the CLI:

```text
~/.config/remarkable-cli/auth.json
~/.config/remarkable-cli/config.json
~/.config/remarkable-cli/state.json
```

### `config.json`

User-editable configuration.

Current supported keys:

```json
{
  "syncFolder": "~/remarkable",
  "excludePdfs": false
}
```

Key reference:

- `syncFolder`: local destination folder for synced notes. `~/...` is expanded to your home directory.
- `excludePdfs`: if `true`, documents whose reMarkable `fileType` is `pdf` are skipped during sync.

Example:

```json
{
  "syncFolder": "~/Documents/remarkable",
  "excludePdfs": true
}
```

Important limits:

- There is currently no config key to choose output formats.
- Markdown output is always written.
- PDF output is written when the document has a source PDF or handwritten pages that can be rendered to PDF.
- EPUB attachments are written when the source document includes an EPUB.

### `auth.json`

Stores device registration state:

```json
{
  "deviceToken": "...",
  "deviceId": "..."
}
```

This file is created by `init`. `reset --auth` clears it by writing empty values.

### `state.json`

Stores sync metadata and the last sync report.

Typical shape:

```json
{
  "lastSyncTimestamp": 0,
  "syncState": {}
}
```

What it is used for:

- Tracking which cloud items have already been synced
- Detecting unchanged documents
- Reusing local documents when render metadata matches
- Showing `status`

You normally do not need to edit this file manually. Use `remarkable-cli reset` instead.

## How It Works

The authentication model is a two-step reMarkable cloud flow:

1. You open the reMarkable device connect page and sign in.
2. reMarkable gives you a one-time code.
3. `remarkable-cli init` sends that code plus a generated local device ID to the reMarkable device registration endpoint.
4. reMarkable returns a device token.
5. The CLI stores the device token and device ID in `~/.config/remarkable-cli/auth.json`.
6. On each sync, the CLI exchanges the stored device token for a user token.
7. That user token is then used to access the reMarkable sync endpoints and download document data.

Storage behavior:

- The device token is persisted locally.
- The user token is refreshed when needed during sync and is not stored in the config files.
- `reset --auth` disconnects the machine by clearing the stored device token and device ID.

Cloud access behavior:

- The CLI reads the cloud root entry list.
- Each item is identified as either a document or a collection.
- Deleted items are ignored.
- Collections define the local folder structure.
- Documents are downloaded and rendered into local files.

## Output Formats

The sync folder contains one Markdown file per synced document plus an `attachments/` directory for binary files.

### Folder layout

Collections on reMarkable become local folders. A document named `Meeting Notes` inside a reMarkable folder `Work` becomes:

```text
<syncFolder>/Work/Meeting Notes.md
<syncFolder>/Work/attachments/Meeting Notes.pdf
```

Document and folder names are sanitized for local filesystem use.

### Markdown

For each synced document, the CLI writes:

```text
<document folder>/<document name>.md
```

The Markdown file contains:

- YAML frontmatter
- The reMarkable document ID
- Last modified timestamp from reMarkable
- Render version
- Page count
- File type
- EPUB attachment path when present
- An embedded image link to the PDF when a PDF was written

Example structure:

```markdown
---
title: "Meeting Notes"
remarkable_id: "..."
last_modified: "..."
render_version: 4
page_count: 3
file_type: "notebook"
---

![Meeting Notes](attachments/Meeting Notes.pdf#page=1)

*3 pages synced from reMarkable*
```

### PDF

PDF output is written to:

```text
<document folder>/attachments/<document name>.pdf
```

How PDFs are produced:

- If the reMarkable document already has a base PDF and annotations, the CLI overlays highlights and strokes onto that PDF.
- If the document has a base PDF and no annotations, the original PDF is written as-is.
- If the document is a handwritten notebook with `.rm` pages and no base PDF, the CLI renders those pages into a generated PDF.

When `excludePdfs` is `true`, documents whose reMarkable `fileType` is `pdf` are skipped completely.

### EPUB

If a document includes an EPUB source file, the CLI writes:

```text
<document folder>/attachments/<document name>.epub
```

The Markdown frontmatter also includes an `epub` field pointing to that attachment.

### Raw `.rm` page files

For each downloaded page with raw reMarkable stroke data, the CLI writes:

```text
<document folder>/attachments/<document name>_<page-id>.rm
```

These are the raw page files downloaded from the cloud for that document.

### Sync Log

Each sync folder also gets a log file:

```text
<syncFolder>/.sync-log.md
```

This file contains recent sync reports, newest first, including:

- Duration
- Cloud item count
- Listed count
- Synced count
- Unchanged count
- Failed count
- Fatal error, if any
- Failed item details

## Operational Notes

- Node.js `18+` is required.
- The CLI stores auth and state locally under `~/.config/remarkable-cli/`.
- The sync folder is created automatically if it does not exist.
- Deleted reMarkable items are skipped and removed from local sync state tracking.
- The tool tracks render version internally. If the render version changes in a future release, a later sync may re-render local documents.
- Sync progress is shown live in the terminal while work is running.

## Troubleshooting

### `Not registered. Run remarkable-cli init`

Run:

```bash
remarkable-cli init
```

### Registration failed

The one-time code may be wrong or expired. Run `remarkable-cli init` again and enter a fresh code from the reMarkable connect page.

### Next sync should re-download everything

Run:

```bash
remarkable-cli reset
remarkable-cli sync
```

### Disconnect this machine completely

Run:

```bash
remarkable-cli reset --auth
```
