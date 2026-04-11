# remarkable

Rust CLI for syncing reMarkable documents to a local folder.

## Commands

```bash
remarkable init
remarkable sync [--force] [--dry-run]
remarkable status
remarkable reset [--auth]
```

Running `remarkable` without a subcommand starts `init` when the device is not registered and `sync` when it is.

## Config

The CLI stores state in `~/.config/remarkable/`:

```text
~/.config/remarkable/config.toml
~/.config/remarkable/auth.toml
~/.config/remarkable/state.toml
```

Default `config.toml`:

```toml
sync_folder = "~/remarkable-notes"
output_format = "pdf"
```

`output_format` may be `pdf` or `markdown`.

## Sync behavior

- Uses the reMarkable device registration and token refresh APIs.
- Walks the cloud root tree and resolves documents plus collections.
- Mirrors collection structure into the local sync folder.
- Downloads changed documents only, unless `--force` is used.
- Supports real `--dry-run` output without modifying local files or sync state.
- Writes a sync log to `<sync_folder>/.sync-log.md`.
- Stores EPUB and raw `.rm` page files in per-document `attachments/` folders.

## Build

```bash
cargo build --release
```
