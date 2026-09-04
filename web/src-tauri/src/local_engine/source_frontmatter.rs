//! Source concept frontmatter written at ingest time.
//!
//! OKF only requires `type`. CoWiki adds resource provenance so a Source can
//! be traced back to its origin without treating the body as instructions.

use std::collections::HashSet;

use sha2::{Digest, Sha256};

use crate::extract::ExtractResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFrontmatter {
    pub title: String,
    pub resource: Option<String>,
    pub requested_url: Option<String>,
    pub final_url: Option<String>,
    pub timestamp: String,
    pub source_hash: Option<String>,
    pub content_hash: String,
    pub warnings: Vec<String>,
}

impl SourceFrontmatter {
    pub fn for_file(
        title: impl Into<String>,
        original_name: &str,
        body: &str,
        source_hash: impl Into<String>,
        result: &ExtractResult,
    ) -> Self {
        Self {
            title: title.into(),
            resource: Some(original_name.to_string()),
            requested_url: None,
            final_url: None,
            timestamp: utc_timestamp_now(),
            source_hash: Some(source_hash.into()),
            content_hash: hash_content(body),
            warnings: warning_codes(result),
        }
    }

    pub fn for_paste(title: impl Into<String>, resource: &str, body: &str) -> Self {
        Self {
            title: title.into(),
            resource: Some(resource.to_string()),
            requested_url: None,
            final_url: None,
            timestamp: utc_timestamp_now(),
            source_hash: None,
            content_hash: hash_content(body),
            warnings: Vec::new(),
        }
    }

    pub fn for_web(requested_url: &str, result: &ExtractResult) -> Self {
        let requested = requested_url.trim();
        let final_url = result
            .stats
            .web
            .as_ref()
            .and_then(|web| web.final_url.clone())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| requested.to_string());
        let title = result
            .document
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| requested.to_string());
        Self {
            title,
            resource: Some(requested.to_string()),
            requested_url: Some(requested.to_string()),
            final_url: Some(final_url),
            timestamp: utc_timestamp_now(),
            source_hash: None,
            content_hash: hash_content(result.output_text()),
            warnings: warning_codes(result),
        }
    }

    /// Save a URL as the Source body when article extraction is unavailable.
    pub fn for_url_reference(url: &str) -> Self {
        let requested = url.trim();
        Self {
            title: requested.to_string(),
            resource: Some(requested.to_string()),
            requested_url: Some(requested.to_string()),
            final_url: None,
            timestamp: utc_timestamp_now(),
            source_hash: None,
            content_hash: hash_content(requested),
            warnings: Vec::new(),
        }
    }

    pub fn render_document(&self, body_text: &str) -> String {
        let body = body_text.trim();
        let mut meta = self.clone();
        meta.content_hash = hash_content(body);
        format!("---\n{}---\n\n{}\n", meta.to_yaml(), body)
    }

    pub fn to_yaml(&self) -> String {
        let mut out = String::from("type: Source\n");
        push_quoted(&mut out, "title", &self.title);
        if let Some(resource) = &self.resource {
            push_quoted(&mut out, "resource", resource);
        }
        if let Some(url) = &self.requested_url {
            push_quoted(&mut out, "requested_url", url);
        }
        if let Some(url) = &self.final_url {
            push_quoted(&mut out, "final_url", url);
        }
        if let Some(url) = self
            .final_url
            .as_ref()
            .or(self.requested_url.as_ref())
        {
            push_quoted(&mut out, "source_url", url);
        }
        push_quoted(&mut out, "timestamp", &self.timestamp);
        if let Some(hash) = &self.source_hash {
            push_quoted(&mut out, "source_hash", hash);
        }
        push_quoted(&mut out, "content_hash", &self.content_hash);
        push_warnings(&mut out, &self.warnings);
        out
    }
}

pub fn hash_content(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.trim().as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn warning_codes(result: &ExtractResult) -> Vec<String> {
    let mut seen = HashSet::new();
    result
        .diagnostics
        .iter()
        .filter(|item| seen.insert(item.code))
        .map(|item| item.code.as_str().to_string())
        .collect()
}

fn utc_timestamp_now() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

fn push_quoted(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(": ");
    out.push_str(&yaml_string(value));
    out.push('\n');
}

fn push_warnings(out: &mut String, warnings: &[String]) {
    if warnings.is_empty() {
        out.push_str("warnings: []\n");
        return;
    }
    out.push_str("warnings:\n");
    for warning in warnings {
        out.push_str("  - ");
        out.push_str(warning);
        out.push('\n');
    }
}

fn yaml_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', " ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{
        Diagnostic, DiagnosticCode, ExtractResult, ExtractStats, Provenance, SourceFormat,
        WebStats, BUILTIN_EXTRACTOR,
    };
    use serde_yaml::{Mapping, Value};

    fn parsed(yaml: &str) -> Mapping {
        serde_yaml::from_str(yaml).expect("Source frontmatter must be parseable YAML")
    }

    fn string_field(mapping: &Mapping, key: &str) -> String {
        mapping
            .get(Value::String(key.into()))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{key} should be a string"))
            .to_string()
    }

    fn sample_result(text: &str) -> ExtractResult {
        let mut result = ExtractResult::from_text(
            text,
            SourceFormat::Text,
            Provenance::local_fast(BUILTIN_EXTRACTOR, "0.1.0", 4),
        );
        result.diagnostics.push(Diagnostic::new(
            DiagnosticCode::OutputTooShort,
            "fixture warning",
        ));
        result
    }

    #[test]
    fn file_frontmatter_records_hashes_and_warnings() {
        let body = "Extracted report body that is long enough to keep.";
        let meta = SourceFrontmatter::for_file(
            "report",
            "report.pdf",
            body,
            "abc123",
            &sample_result(body),
        );
        let yaml = meta.to_yaml();
        let mapping = parsed(&yaml);
        assert_eq!(string_field(&mapping, "type"), "Source");
        assert_eq!(string_field(&mapping, "title"), "report");
        assert_eq!(string_field(&mapping, "resource"), "report.pdf");
        assert_eq!(string_field(&mapping, "source_hash"), "abc123");
        assert_eq!(string_field(&mapping, "content_hash"), hash_content(body));
        assert!(mapping.get(Value::String("extractor".into())).is_none());
        assert!(mapping.get(Value::String("requested_url".into())).is_none());
        assert!(mapping.get(Value::String("final_url".into())).is_none());
        let warnings = mapping
            .get(Value::String("warnings".into()))
            .and_then(Value::as_sequence)
            .expect("warnings list");
        assert_eq!(
            warnings
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>(),
            ["OUTPUT_TOO_SHORT"]
        );
    }

    #[test]
    fn url_frontmatter_keeps_requested_and_final_url() {
        let url = "https://example.com/article";
        let mut result = ExtractResult::from_adapter(
            "Exponential backoff keeps a struggling dependency from being stampeded.",
            SourceFormat::Url,
            ExtractStats {
                web: Some(WebStats {
                    final_url: Some("https://www.example.com/article".into()),
                    ..WebStats::default()
                }),
                ..ExtractStats::default()
            },
            Provenance::local_fast("readabilityrs+htmd", "0.1.0", 12),
        );
        result.document.title = Some("Retry Patterns".into());
        result.diagnostics.push(Diagnostic::new(
            DiagnosticCode::WebNavigationDensityHigh,
            "fixture warning",
        ));
        let mut meta = SourceFrontmatter::for_web(url, &result);
        meta.timestamp = "2026-09-03T05:00:00Z".into();
        let mapping = parsed(&meta.to_yaml());
        assert_eq!(string_field(&mapping, "title"), "Retry Patterns");
        assert_eq!(string_field(&mapping, "resource"), url);
        assert_eq!(string_field(&mapping, "requested_url"), url);
        assert_eq!(
            string_field(&mapping, "final_url"),
            "https://www.example.com/article"
        );
        assert!(mapping.get(Value::String("source_hash".into())).is_none());
        let warnings = mapping
            .get(Value::String("warnings".into()))
            .and_then(Value::as_sequence)
            .expect("warnings list");
        assert_eq!(
            warnings
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>(),
            ["WEB_NAVIGATION_DENSITY_HIGH"]
        );
    }

    #[test]
    fn url_reference_frontmatter_keeps_the_link_only() {
        let url = "https://example.com/article";
        let meta = SourceFrontmatter::for_url_reference(url);
        let mapping = parsed(&meta.to_yaml());
        assert_eq!(string_field(&mapping, "title"), url);
        assert_eq!(string_field(&mapping, "resource"), url);
        assert_eq!(string_field(&mapping, "requested_url"), url);
        assert!(mapping.get(Value::String("final_url".into())).is_none());
        assert_eq!(string_field(&mapping, "content_hash"), hash_content(url));
    }

    #[test]
    fn paste_frontmatter_omits_web_and_source_hash_fields() {
        let meta = SourceFrontmatter::for_paste("Note", "notes.md", "Pasted notes.");
        let mapping = parsed(&meta.to_yaml());
        assert_eq!(string_field(&mapping, "resource"), "notes.md");
        assert!(mapping.get(Value::String("requested_url".into())).is_none());
        assert!(mapping.get(Value::String("source_hash".into())).is_none());
    }

    #[test]
    fn render_document_hashes_the_trimmed_body() {
        let meta = SourceFrontmatter::for_paste("Note", "Note", "  body text  ");
        let document = meta.render_document("  body text  ");
        assert!(document.starts_with("---\n"));
        assert!(document.contains("type: Source\n"));
        assert!(document.ends_with("---\n\nbody text\n") || document.contains("\n\nbody text\n"));
        assert!(document.contains(&hash_content("body text")));
        let mapping = parsed(document.split("---\n").nth(1).unwrap());
        assert_eq!(string_field(&mapping, "title"), "Note");
    }
}
