# remarkable

Sync your reMarkable tablet to a local folder.

## Install

```bash
brew tap ComputelessComputer/remarkable
brew install remarkable
```

## Quick start

```bash
remarkable auth login   # connect your tablet (one-time)
remarkable sync         # sync documents to ~/remarkable-notes
```

---

## Commands

### `remarkable auth login`

Connect your reMarkable tablet for the first time.

Opens the reMarkable registration page, prompts you for the one-time code, and stores a device token in `~/.config/remarkable/auth.toml`. You only need to do this once.

### `remarkable auth refresh`

Renew your device token without re-registering. Use this if syncing starts returning authentication errors.

### `remarkable auth logout`

Disconnect the device and delete the stored token. After this, run `remarkable auth login` to reconnect.

---

### `remarkable sync`

Download new and changed documents from the reMarkable cloud to your local sync folder.

Documents are saved as PDF by default, preserving the folder structure from your tablet. Only changed documents are downloaded — unchanged ones are skipped.

**Flags:**

- `--force` — re-download all documents, even unchanged ones
- `--dry-run` — show what would sync without downloading anything

---

### `remarkable status`

Show whether the device is connected, when it was last synced, how many documents were synced, and where they're saved.

---

### `remarkable reset`

Clear the sync state so the next `sync` re-downloads everything.

**Flags:**

- `--auth` — also delete the device token (full disconnect, same as `auth logout`)

---

### Default behavior

Running `remarkable` with no subcommand:
- Runs `auth login` if no device token is stored
- Runs `sync` if a device token exists

---

## Config

Stored in `~/.config/remarkable/config.toml`. Created automatically on first run with defaults.

```toml
sync_folder = "~/remarkable-notes"
output_format = "pdf"    # or "markdown"
```

**`sync_folder`** — where documents are saved. Supports `~`.

**`output_format`** — `pdf` keeps your handwritten pages as-is. `markdown` exports text where available.

---

## Files

```
~/.config/remarkable/
  auth.toml      # device token
  config.toml    # your settings
  state.toml     # sync state (which documents have been downloaded)
```

The sync folder also gets a `.sync-log.md` with a record of every sync run.
