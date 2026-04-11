use crate::client::{RawEntry, RemarkableClient};
use crate::config::{
    AppConfig, OutputFormat, SyncFailure, SyncRecord, SyncReport, SyncState, RENDER_VERSION,
};
use crate::error::{AppError, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

const SYNC_LOG_NAME: &str = ".sync-log.md";
const LOG_HEADER: &str = "# remarkable Sync Log\n\nNewest entries appear first.\n";
const MAX_LOG_ENTRIES: usize = 50;

#[derive(Debug, Clone, Deserialize)]
struct ItemMetadata {
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    #[serde(rename = "lastModified")]
    last_modified: String,
    #[serde(default)]
    parent: String,
    #[serde(rename = "type")]
    item_type: String,
    #[serde(rename = "visibleName")]
    visible_name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DocumentContent {
    #[serde(default, rename = "fileType")]
    file_type: String,
    #[serde(default)]
    pages: Vec<String>,
    #[serde(default, rename = "pageCount")]
    page_count: usize,
}

#[derive(Debug, Clone)]
struct RemarkableItem {
    id: String,
    hash: String,
    visible_name: String,
    last_modified: String,
    parent: String,
    item_type: String,
    file_entries: Vec<RawEntry>,
}

#[derive(Debug)]
struct SyncEvent {
    id: String,
    name: String,
    action: SyncAction,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncAction {
    Listed,
    Synced,
    Skipped,
    Failed,
}

#[derive(Debug)]
pub struct SyncOutcome {
    pub report: SyncReport,
    pub dry_run_items: Vec<String>,
}

pub struct SyncEngine {
    client: RemarkableClient,
    config: AppConfig,
    state: SyncState,
    sync_root: PathBuf,
    started_at: u64,
    events: Vec<SyncEvent>,
}

impl SyncEngine {
    pub fn new(
        client: RemarkableClient,
        config: AppConfig,
        state: SyncState,
        sync_root: PathBuf,
    ) -> Self {
        Self {
            client,
            config,
            state,
            sync_root,
            started_at: epoch_seconds(),
            events: Vec::new(),
        }
    }

    pub fn into_state(self) -> SyncState {
        self.state
    }

    pub async fn sync(&mut self, force: bool, dry_run: bool) -> Result<SyncOutcome> {
        if force {
            self.state = SyncState::default();
        }

        if !dry_run {
            fs::create_dir_all(&self.sync_root).await?;
        }

        let progress = ProgressDisplay::new()?;
        progress.spinner.set_message("Authenticating");
        self.client.refresh_user_token().await?;

        progress.spinner.set_message("Listing cloud items");
        let root = self.client.get_root_hash().await?;
        let root_entries = self.client.get_entries(&root.hash).await?;

        let mut all_items = BTreeMap::<String, RemarkableItem>::new();
        let mut documents_to_sync = Vec::<RemarkableItem>::new();
        let mut dry_run_items = Vec::<String>::new();
        let mut cloud_ids = BTreeSet::<String>::new();

        for (index, entry) in root_entries.entries.iter().enumerate() {
            cloud_ids.insert(entry.id.clone());
            progress.spinner.set_message(format!(
                "Listing cloud items ({}/{})",
                index + 1,
                root_entries.entries.len()
            ));

            let item = self.resolve_item(entry, force).await?;
            if let Some(item) = item {
                self.events.push(SyncEvent {
                    id: item.id.clone(),
                    name: item.visible_name.clone(),
                    action: SyncAction::Listed,
                    error: None,
                });
                all_items.insert(item.id.clone(), item.clone());

                if item.item_type == "DocumentType" {
                    let should_sync = force || self.needs_sync(&item).await?;
                    if should_sync {
                        if dry_run {
                            dry_run_items.push(item.visible_name.clone());
                        }
                        documents_to_sync.push(item);
                    } else {
                        self.events.push(SyncEvent {
                            id: item.id.clone(),
                            name: item.visible_name.clone(),
                            action: SyncAction::Skipped,
                            error: None,
                        });
                    }
                }
            }
        }

        progress.docs.set_length(documents_to_sync.len() as u64);
        for document in &documents_to_sync {
            progress
                .spinner
                .set_message(format!("Syncing {}", document.visible_name));

            if dry_run {
                progress.docs.inc(1);
                continue;
            }

            match self.sync_document(document, &all_items).await {
                Ok(()) => {
                    self.events.push(SyncEvent {
                        id: document.id.clone(),
                        name: document.visible_name.clone(),
                        action: SyncAction::Synced,
                        error: None,
                    });
                }
                Err(err) => {
                    self.events.push(SyncEvent {
                        id: document.id.clone(),
                        name: document.visible_name.clone(),
                        action: SyncAction::Failed,
                        error: Some(err.to_string()),
                    });
                }
            }
            progress.docs.inc(1);
        }

        if !dry_run {
            self.state.records.retain(|id, _| cloud_ids.contains(id));
        }

        progress.finish();
        let mut report = self.build_report(root_entries.entries.len());
        if dry_run {
            report.synced_count = 0;
            report.skipped_count = documents_to_sync.len();
            report.log_path = None;
            return Ok(SyncOutcome {
                report,
                dry_run_items,
            });
        }

        report.log_path = Some(self.sync_root.join(SYNC_LOG_NAME).display().to_string());
        self.state.last_sync_timestamp = Some(report.end_time);
        self.state.last_sync_report = Some(report.clone());
        self.write_log(&report).await?;
        Ok(SyncOutcome {
            report,
            dry_run_items,
        })
    }

    async fn resolve_item(
        &mut self,
        entry: &RawEntry,
        force: bool,
    ) -> Result<Option<RemarkableItem>> {
        if !force {
            if let Some(cached) = self.state.records.get(&entry.id) {
                if cached.hash == entry.hash
                    && !cached.visible_name.is_empty()
                    && !cached.item_type.is_empty()
                {
                    return Ok(Some(RemarkableItem {
                        id: entry.id.clone(),
                        hash: entry.hash.clone(),
                        visible_name: cached.visible_name.clone(),
                        last_modified: cached.last_modified.clone(),
                        parent: cached.parent.clone(),
                        item_type: cached.item_type.clone(),
                        file_entries: Vec::new(),
                    }));
                }
            }
        }

        let item_entries = self.client.get_entries(&entry.hash).await?;
        let metadata_entry = item_entries
            .entries
            .iter()
            .find(|child| child.id.ends_with(".metadata"))
            .ok_or_else(|| {
                AppError::InvalidResponse(format!("missing metadata for {}", entry.id))
            })?;
        let metadata_text = self.client.get_text_by_hash(&metadata_entry.hash).await?;
        let metadata: ItemMetadata = serde_json::from_str(&metadata_text)?;

        if metadata.deleted {
            return Ok(None);
        }
        if metadata.item_type != "DocumentType" && metadata.item_type != "CollectionType" {
            return Ok(None);
        }

        Ok(Some(RemarkableItem {
            id: entry.id.clone(),
            hash: entry.hash.clone(),
            visible_name: metadata.visible_name,
            last_modified: metadata.last_modified,
            parent: metadata.parent,
            item_type: metadata.item_type,
            file_entries: item_entries.entries,
        }))
    }

    async fn needs_sync(&self, item: &RemarkableItem) -> Result<bool> {
        let Some(record) = self.state.records.get(&item.id) else {
            return Ok(true);
        };

        if record.hash != item.hash
            || record.last_modified != item.last_modified
            || record.output_format != self.config.output_format
            || record.render_version != RENDER_VERSION
        {
            return Ok(true);
        }

        let output_path = output_path(
            Path::new(&record.local_path),
            &sanitize_filename(&item.visible_name),
            &self.config.output_format,
        );
        Ok(fs::metadata(output_path).await.is_err())
    }

    async fn sync_document(
        &mut self,
        item: &RemarkableItem,
        all_items: &BTreeMap<String, RemarkableItem>,
    ) -> Result<()> {
        let content = self.load_content(item).await?;
        let folder = self.document_folder(item, all_items);
        let attachments = folder.join("attachments");
        fs::create_dir_all(&attachments).await?;

        let safe_name = sanitize_filename(&item.visible_name);
        let pdf_entry = item
            .file_entries
            .iter()
            .find(|entry| entry.id.ends_with(".pdf"));
        let epub_entry = item
            .file_entries
            .iter()
            .find(|entry| entry.id.ends_with(".epub"));
        let rm_entries: Vec<&RawEntry> = item
            .file_entries
            .iter()
            .filter(|entry| entry.id.ends_with(".rm"))
            .collect();

        let mut pdf_attachment_rel = None;
        if let Some(pdf_entry) = pdf_entry {
            let bytes = self.client.get_binary_by_hash(&pdf_entry.hash).await?;
            let pdf_path = attachments.join(format!("{safe_name}.pdf"));
            fs::write(&pdf_path, bytes).await?;
            pdf_attachment_rel = Some(format!("attachments/{safe_name}.pdf"));
        } else if !rm_entries.is_empty() {
            let placeholder = build_placeholder_pdf(
                &item.visible_name,
                content.page_count.max(rm_entries.len()),
                &content,
            );
            let pdf_path = attachments.join(format!("{safe_name}.pdf"));
            fs::write(&pdf_path, placeholder).await?;
            pdf_attachment_rel = Some(format!("attachments/{safe_name}.pdf"));
        }

        if let Some(epub_entry) = epub_entry {
            let bytes = self.client.get_binary_by_hash(&epub_entry.hash).await?;
            fs::write(attachments.join(format!("{safe_name}.epub")), bytes).await?;
        }

        for entry in rm_entries {
            let bytes = self.client.get_binary_by_hash(&entry.hash).await?;
            let page_id = entry
                .id
                .rsplit('/')
                .next()
                .unwrap_or(&entry.id)
                .trim_end_matches(".rm");
            fs::write(attachments.join(format!("{safe_name}_{page_id}.rm")), bytes).await?;
        }

        match self.config.output_format {
            OutputFormat::Pdf => {
                let final_pdf = folder.join(format!("{safe_name}.pdf"));
                if let Some(rel) = &pdf_attachment_rel {
                    fs::copy(folder.join(rel), &final_pdf).await?;
                } else {
                    let placeholder = build_placeholder_pdf(
                        &item.visible_name,
                        content.page_count.max(1),
                        &content,
                    );
                    fs::write(final_pdf, placeholder).await?;
                }
            }
            OutputFormat::Markdown => {
                let markdown = build_markdown(item, &content, pdf_attachment_rel.as_deref());
                fs::write(folder.join(format!("{safe_name}.md")), markdown).await?;
            }
        }

        self.state.records.insert(
            item.id.clone(),
            SyncRecord {
                hash: item.hash.clone(),
                last_modified: item.last_modified.clone(),
                local_path: folder.display().to_string(),
                visible_name: item.visible_name.clone(),
                parent: item.parent.clone(),
                item_type: item.item_type.clone(),
                output_format: self.config.output_format.clone(),
                render_version: RENDER_VERSION,
            },
        );

        Ok(())
    }

    async fn load_content(&mut self, item: &RemarkableItem) -> Result<DocumentContent> {
        let Some(content_entry) = item
            .file_entries
            .iter()
            .find(|entry| entry.id.ends_with(".content"))
        else {
            return Ok(DocumentContent::default());
        };

        let text = self.client.get_text_by_hash(&content_entry.hash).await?;
        match serde_json::from_str::<DocumentContent>(&text) {
            Ok(content) => Ok(content),
            Err(_) => Ok(DocumentContent::default()),
        }
    }

    fn document_folder(
        &self,
        item: &RemarkableItem,
        all_items: &BTreeMap<String, RemarkableItem>,
    ) -> PathBuf {
        let mut parts = Vec::<String>::new();
        let mut current = item.parent.clone();

        while !current.is_empty() && current != "trash" {
            let Some(parent) = all_items.get(&current) else {
                break;
            };
            parts.push(sanitize_filename(&parent.visible_name));
            current = parent.parent.clone();
        }

        parts.reverse();
        let mut path = self.sync_root.clone();
        for part in parts {
            path.push(part);
        }
        path
    }

    fn build_report(&self, cloud_item_count: usize) -> SyncReport {
        let end_time = epoch_seconds();
        let failed_items = self
            .events
            .iter()
            .filter(|event| event.action == SyncAction::Failed)
            .map(|event| SyncFailure {
                id: event.id.clone(),
                name: event.name.clone(),
                error: event
                    .error
                    .clone()
                    .unwrap_or_else(|| "unknown error".into()),
            })
            .collect::<Vec<_>>();

        SyncReport {
            start_time: self.started_at,
            end_time,
            duration_ms: (end_time.saturating_sub(self.started_at)) * 1_000,
            cloud_item_count,
            listed_count: self
                .events
                .iter()
                .filter(|event| event.action == SyncAction::Listed)
                .count(),
            synced_count: self
                .events
                .iter()
                .filter(|event| event.action == SyncAction::Synced)
                .count(),
            skipped_count: self
                .events
                .iter()
                .filter(|event| event.action == SyncAction::Skipped)
                .count(),
            failed_count: failed_items.len(),
            failed_items,
            fatal_error: None,
            log_path: None,
        }
    }

    async fn write_log(&self, report: &SyncReport) -> Result<()> {
        let log_path = self.sync_root.join(SYNC_LOG_NAME);
        let entry = format_log_entry(report);

        let existing = match fs::read_to_string(&log_path).await {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err.into()),
        };

        let sections = split_log_sections(&existing);
        let mut content = String::from(LOG_HEADER);
        content.push_str(&entry);
        for section in sections.into_iter().take(MAX_LOG_ENTRIES.saturating_sub(1)) {
            content.push('\n');
            content.push_str(&section);
        }
        fs::write(log_path, content).await?;
        Ok(())
    }
}

struct ProgressDisplay {
    _multi: MultiProgress,
    spinner: ProgressBar,
    docs: ProgressBar,
}

impl ProgressDisplay {
    fn new() -> Result<Self> {
        let multi = MultiProgress::new();
        let spinner = multi.add(ProgressBar::new_spinner());
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg}")
                .map_err(|err| AppError::Config(err.to_string()))?,
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        let docs = multi.add(ProgressBar::new(0));
        docs.set_style(
            ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} documents")
                .map_err(|err| AppError::Config(err.to_string()))?,
        );

        Ok(Self {
            _multi: multi,
            spinner,
            docs,
        })
    }

    fn finish(&self) {
        self.spinner.finish_and_clear();
        self.docs.finish_and_clear();
    }
}

fn output_path(folder: &Path, safe_name: &str, format: &OutputFormat) -> PathBuf {
    folder.join(format!("{safe_name}.{}", format.extension()))
}

fn sanitize_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>();
    sanitized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn build_markdown(
    item: &RemarkableItem,
    content: &DocumentContent,
    pdf_path: Option<&str>,
) -> String {
    let page_count = content.page_count.max(content.pages.len());
    let mut output = String::new();
    let _ = writeln!(output, "---");
    let _ = writeln!(
        output,
        "title: \"{}\"",
        escape_yaml_string(&item.visible_name)
    );
    let _ = writeln!(output, "remarkable_id: \"{}\"", item.id);
    let _ = writeln!(output, "last_modified: \"{}\"", item.last_modified);
    let _ = writeln!(output, "render_version: {}", RENDER_VERSION);
    let _ = writeln!(output, "page_count: {}", page_count);
    let file_type = if content.file_type.is_empty() {
        "notebook"
    } else {
        &content.file_type
    };
    let _ = writeln!(output, "file_type: \"{}\"", escape_yaml_string(file_type));
    let _ = writeln!(output, "---\n");

    if let Some(pdf_path) = pdf_path {
        let _ = writeln!(output, "![{}]({pdf_path}#page=1)\n", item.visible_name);
    }

    let _ = writeln!(
        output,
        "*{} pages synced from reMarkable*",
        page_count.max(1)
    );
    output
}

fn build_placeholder_pdf(title: &str, page_count: usize, content: &DocumentContent) -> Vec<u8> {
    let mut objects = Vec::<String>::new();
    let page_total = page_count.max(1);
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    let mut kids = String::new();
    for index in 0..page_total {
        let page_obj = 4 + index * 2;
        let _ = write!(kids, "{page_obj} 0 R ");
    }
    objects.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {page_total} >>"
    ));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());

    for page in 0..page_total {
        let page_number = page + 1;
        let message = format!(
            "BT /F1 18 Tf 72 760 Td ({}) Tj 0 -28 Td /F1 11 Tf (Synced from reMarkable) Tj 0 -18 Td (Page {} of {}) Tj 0 -18 Td (Source type: {}) Tj ET",
            escape_pdf_text(title),
            page_number,
            page_total,
            escape_pdf_text(if content.file_type.is_empty() {
                "notebook"
            } else {
                &content.file_type
            }),
        );
        let stream = format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            message.len(),
            message
        );
        let page_obj = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            5 + page * 2
        );
        objects.push(page_obj);
        objects.push(stream);
    }

    render_pdf(objects)
}

fn render_pdf(objects: Vec<String>) -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0usize];

    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        let chunk = format!("{} 0 obj\n{}\nendobj\n", index + 1, object);
        bytes.extend_from_slice(chunk.as_bytes());
    }

    let xref_start = bytes.len();
    bytes.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    bytes.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        let line = format!("{offset:010} 00000 n \n");
        bytes.extend_from_slice(line.as_bytes());
    }
    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_start
    );
    bytes.extend_from_slice(trailer.as_bytes());
    bytes
}

fn escape_pdf_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn escape_yaml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn split_log_sections(content: &str) -> Vec<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let body = trimmed
        .strip_prefix(LOG_HEADER.trim_end())
        .unwrap_or(trimmed)
        .trim();
    if body.is_empty() {
        return Vec::new();
    }

    body.split("\n## ")
        .filter_map(|section| {
            let section = section.trim();
            if section.is_empty() {
                None
            } else if section.starts_with("## ") {
                Some(format!("{section}\n"))
            } else {
                Some(format!("## {section}\n"))
            }
        })
        .collect()
}

fn format_log_entry(report: &SyncReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "## {}", report.end_time);
    let _ = writeln!(output);
    let _ = writeln!(output, "- Duration: {}s", report.duration_ms / 1_000);
    let _ = writeln!(output, "- Cloud items: {}", report.cloud_item_count);
    let _ = writeln!(output, "- Listed: {}", report.listed_count);
    let _ = writeln!(output, "- Synced: {}", report.synced_count);
    let _ = writeln!(output, "- Skipped: {}", report.skipped_count);
    let _ = writeln!(output, "- Failed: {}", report.failed_count);
    if !report.failed_items.is_empty() {
        let _ = writeln!(output, "- Failed items:");
        for item in &report.failed_items {
            let _ = writeln!(output, "  - \"{}\" - {}", item.name, item.error);
        }
    }
    output
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_filenames() {
        assert_eq!(sanitize_filename("a/b:c*?"), "a_b_c__");
    }

    #[test]
    fn placeholder_pdf_has_header() {
        let pdf = build_placeholder_pdf("Test", 1, &DocumentContent::default());
        assert!(pdf.starts_with(b"%PDF-1.4"));
    }
}
