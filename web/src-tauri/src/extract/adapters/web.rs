//! Static HTML → Markdown. `readabilityrs` is the default article extractor;
//! a sanitized semantic `body` is the only local fallback. Isolated-browser
//! capture is P5.

use std::collections::HashSet;
use std::time::Instant;

use readabilityrs::{Readability, ReadabilityOptions};
use scraper::{ElementRef, Html, Node, Selector};

use crate::extract::attempt::{AttemptChain, AttemptRecord};
use crate::extract::contract::{
    ExtractResult, ExtractStats, FallbackKind, Provenance, SourceFormat, WebStats,
};
use crate::extract::fetch::FetchedPage;
use crate::extract::quality::{attach_diagnostics, evaluate_quality, WEB_MIN_ARTICLE_CHARS};
use crate::extract::{
    WEB_BODY_EXTRACTOR, WEB_BODY_EXTRACTOR_VERSION, WEB_EXTRACTOR, WEB_EXTRACTOR_VERSION,
};

const SANITIZE_TAGS: &[&str] = &[
    "html",
    "head",
    "body",
    "title",
    "article",
    "section",
    "main",
    "header",
    "footer",
    "nav",
    "aside",
    "figure",
    "figcaption",
    "picture",
    "pre",
    "code",
    "blockquote",
    "table",
    "thead",
    "tbody",
    "tr",
    "th",
    "td",
    "hr",
    "br",
    "p",
    "div",
    "span",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "strong",
    "em",
    "b",
    "i",
    "u",
    "s",
    "sup",
    "sub",
    "img",
    "a",
];

pub fn extract_html_file(html: &str) -> Result<ExtractResult, String> {
    extract_web_page(
        FetchedPage {
            requested_url: String::new(),
            final_url: String::new(),
            status: 200,
            content_type: "text/html".to_string(),
            html: html.to_string(),
            response_bytes: html.len() as u64,
        },
        SourceFormat::Html,
    )
}

pub fn extract_web_page(page: FetchedPage, format: SourceFormat) -> Result<ExtractResult, String> {
    let page_stats = measure_page(&page.html, &page);
    let mut chain = AttemptChain::new();

    let readability = attempt_readability(format, &page.html, page_url(&page), &page_stats)?;
    let readability_report = evaluate_quality(&readability);
    chain
        .record(AttemptRecord::from_result(
            readability,
            readability_report.clone(),
        ))
        .map_err(|error| error.to_string())?;

    if !readability_report.outcome.is_usable()
        && readability_report
            .suggested_fallback
            .as_ref()
            .is_some_and(|target| target.kind == FallbackKind::SemanticBody)
    {
        let body = attempt_semantic_body(format, &sanitize_html(&page.html), &page_stats)?;
        let body_report = evaluate_quality(&body);
        chain
            .record(AttemptRecord::from_result(body, body_report))
            .map_err(|error| error.to_string())?;
    }

    finish_chain(chain)
}

fn page_url(page: &FetchedPage) -> Option<&str> {
    let url = page.final_url.trim();
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

fn finish_chain(chain: AttemptChain) -> Result<ExtractResult, String> {
    let outcome = chain.finish();
    let Some(selected) = outcome.selected() else {
        let last = outcome
            .attempts
            .last()
            .ok_or_else(|| "web extraction produced no attempts".to_string())?;
        return Err(unusable_message(&last.quality));
    };
    let mut result = selected.result.clone();
    attach_diagnostics(&mut result, &selected.quality);
    Ok(result)
}

fn unusable_message(report: &crate::extract::contract::QualityReport) -> String {
    if report
        .reasons
        .iter()
        .any(|reason| reason.code == crate::extract::contract::DiagnosticCode::WebSpaShellDetected)
        || report
            .suggested_fallback
            .as_ref()
            .is_some_and(|target| target.kind == FallbackKind::IsolatedBrowser)
    {
        return "this page looks like a JavaScript app and needs dynamic capture; static extraction found no article"
            .to_string();
    }
    if report
        .reasons
        .iter()
        .any(|reason| reason.code == crate::extract::contract::DiagnosticCode::WebLoginOrChallenge)
    {
        return "this page looks like a login or challenge wall; CoWiki does not submit credentials during static extraction"
            .to_string();
    }
    let codes: Vec<_> = report
        .reasons
        .iter()
        .map(|reason| reason.code.as_str())
        .collect();
    format!(
        "could not extract a reliable article from this page ({})",
        codes.join(", ")
    )
}

fn attempt_readability(
    format: SourceFormat,
    html: &str,
    url: Option<&str>,
    page_stats: &PageMetrics,
) -> Result<ExtractResult, String> {
    let started = Instant::now();
    let (title, article_html) = extract_article_html(html, url);
    let markdown = html_to_markdown(&sanitize_html(&article_html))?;
    let stats = page_stats.stats_for_article(&markdown, !markdown.trim().is_empty());
    Ok(into_result(
        markdown,
        format,
        title.or_else(|| document_title(html)),
        stats,
        provenance(
            WEB_EXTRACTOR,
            WEB_EXTRACTOR_VERSION,
            "readabilityrs",
            started,
        ),
    ))
}

/// Mozilla Readability via `readabilityrs`. Runs on the unsanitized HTML so
/// class/id/JSON-LD scoring still works; the article fragment is sanitized
/// before Markdown conversion.
fn extract_article_html(html: &str, url: Option<&str>) -> (Option<String>, String) {
    let options = readability_options();
    let article = parse_article(html, url, options.clone())
        .or_else(|| url.and_then(|_| parse_article(html, None, options)));
    match article {
        Some(article) => (
            nonempty_title(article.title),
            article.content.unwrap_or_default(),
        ),
        None => (document_title(html), String::new()),
    }
}

fn readability_options() -> ReadabilityOptions {
    ReadabilityOptions::builder()
        .char_threshold(WEB_MIN_ARTICLE_CHARS as usize)
        .output_markdown(false)
        .build()
}

fn parse_article(
    html: &str,
    url: Option<&str>,
    options: ReadabilityOptions,
) -> Option<readabilityrs::Article> {
    Readability::new(html, url, Some(options))
        .ok()
        .and_then(Readability::parse)
}

fn nonempty_title(title: Option<String>) -> Option<String> {
    title.and_then(|title| {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn attempt_semantic_body(
    format: SourceFormat,
    sanitized: &str,
    page_stats: &PageMetrics,
) -> Result<ExtractResult, String> {
    let started = Instant::now();
    let body_html = semantic_body_html(sanitized);
    let markdown = html_to_markdown(&body_html)?;
    let stats = page_stats.stats_for_article(&markdown, !markdown.trim().is_empty());
    Ok(into_result(
        markdown,
        format,
        document_title(sanitized),
        stats,
        provenance(
            WEB_BODY_EXTRACTOR,
            WEB_BODY_EXTRACTOR_VERSION,
            "semantic-body",
            started,
        ),
    ))
}

fn into_result(
    markdown: String,
    format: SourceFormat,
    title: Option<String>,
    stats: ExtractStats,
    provenance: Provenance,
) -> ExtractResult {
    let mut result = ExtractResult::from_adapter(markdown, format, stats, provenance);
    result.document.title = title;
    result
}

fn provenance(extractor: &str, version: &str, settings: &str, started: Instant) -> Provenance {
    let mut provenance =
        Provenance::local_fast(extractor, version, started.elapsed().as_millis() as u64);
    provenance.settings_hash = settings.to_string();
    provenance
}

fn sanitize_html(html: &str) -> String {
    let tags: HashSet<&str> = SANITIZE_TAGS.iter().copied().collect();
    ammonia::Builder::default()
        .tags(tags)
        .add_tag_attributes("img", ["src", "alt", "title"])
        .add_tag_attributes("a", ["href", "title"])
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .link_rel(None)
        .clean(html)
        .to_string()
}

fn html_to_markdown(html: &str) -> Result<String, String> {
    if html.trim().is_empty() {
        return Ok(String::new());
    }
    htmd::convert(html).map_err(|error| format!("cannot convert HTML to Markdown: {error}"))
}

fn semantic_body_html(html: &str) -> String {
    let document = Html::parse_document(html);
    let raw = ["main", "article", "[role=main]", "body"]
        .iter()
        .find_map(|selector| {
            Selector::parse(selector)
                .ok()
                .and_then(|parsed| document.select(&parsed).next())
                .map(|node| node.html())
        })
        .unwrap_or_else(|| html.to_string());
    let mut tags: HashSet<&str> = SANITIZE_TAGS.iter().copied().collect();
    for chrome in ["nav", "footer", "header", "aside"] {
        tags.remove(chrome);
    }
    ammonia::Builder::default()
        .tags(tags)
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .link_rel(None)
        .clean(&raw)
        .to_string()
}

fn document_title(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|title| title.text().collect::<String>().trim().to_string())
        .filter(|title| !title.is_empty())
}

struct PageMetrics {
    http_status: Option<u16>,
    final_url: Option<String>,
    response_bytes: u64,
    dom_visible_text_chars: u32,
    link_text_ratio: f32,
    spa_shell_detected: bool,
    login_or_challenge_detected: bool,
}

impl PageMetrics {
    fn stats_for_article(&self, markdown: &str, readability_succeeded: bool) -> ExtractStats {
        let article_text_chars = markdown.chars().filter(|ch| !ch.is_whitespace()).count() as u32;
        let article_to_body_text_ratio = if self.dom_visible_text_chars == 0 {
            0.0
        } else {
            article_text_chars as f32 / self.dom_visible_text_chars as f32
        };
        let mut stats = ExtractStats {
            web: Some(WebStats {
                http_status: self.http_status,
                final_url: self.final_url.clone(),
                response_bytes: self.response_bytes,
                article_text_chars,
                article_to_body_text_ratio,
                link_text_ratio: self.link_text_ratio,
                readability_succeeded,
                dom_visible_text_chars: self.dom_visible_text_chars,
                spa_shell_detected: self.spa_shell_detected,
                login_or_challenge_detected: self.login_or_challenge_detected,
            }),
            ..ExtractStats::default()
        };
        if self.dom_visible_text_chars > 0 {
            stats.set_coverage(
                article_text_chars.min(self.dom_visible_text_chars),
                self.dom_visible_text_chars,
            );
        }
        stats
    }
}

fn measure_page(html: &str, page: &FetchedPage) -> PageMetrics {
    let document = Html::parse_document(html);
    let visible = visible_text(&document);
    let link = link_text(&document);
    let visible_chars = visible.chars().filter(|ch| !ch.is_whitespace()).count() as u32;
    let link_chars = link.chars().filter(|ch| !ch.is_whitespace()).count() as u32;
    let link_text_ratio = if visible_chars == 0 {
        0.0
    } else {
        link_chars as f32 / visible_chars as f32
    };
    PageMetrics {
        http_status: Some(page.status),
        final_url: if page.final_url.is_empty() {
            None
        } else {
            Some(page.final_url.clone())
        },
        response_bytes: page.response_bytes,
        dom_visible_text_chars: visible_chars,
        link_text_ratio,
        spa_shell_detected: looks_like_spa(html, visible_chars),
        login_or_challenge_detected: looks_like_login(html, &page.final_url, page.status),
    }
}

fn visible_text(document: &Html) -> String {
    let mut out = String::new();
    collect_visible(document.root_element(), &mut out);
    out
}

fn collect_visible(element: ElementRef<'_>, out: &mut String) {
    if matches!(
        element.value().name(),
        "script" | "style" | "noscript" | "svg" | "template"
    ) {
        return;
    }
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                out.push_str(text);
                out.push(' ');
            }
            Node::Element(_) => {
                if let Some(child_element) = ElementRef::wrap(child) {
                    collect_visible(child_element, out);
                }
            }
            _ => {}
        }
    }
}

fn link_text(document: &Html) -> String {
    let Ok(selector) = Selector::parse("a") else {
        return String::new();
    };
    document
        .select(&selector)
        .map(|link| link.text().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_spa(html: &str, visible_chars: u32) -> bool {
    let lower = html.to_ascii_lowercase();
    let markers = [
        "__next_data__",
        "id=\"__next\"",
        "id=\"root\"",
        "id=\"app\"",
        "__nuxt__",
        "ng-version",
        "data-reactroot",
        "id=\"__nuxt\"",
    ];
    markers.iter().any(|marker| lower.contains(marker)) && visible_chars < 200
}

fn looks_like_login(html: &str, final_url: &str, status: u16) -> bool {
    if matches!(status, 401 | 403) {
        return true;
    }
    let lower = html.to_ascii_lowercase();
    let url = final_url.to_ascii_lowercase();
    url.contains("/login")
        || url.contains("/signin")
        || lower.contains("type=\"password\"")
        || lower.contains("type='password'")
        || lower.contains("g-recaptcha")
        || lower.contains("h-captcha")
        || lower.contains("cf-challenge")
        || lower.contains("id=\"challenge-form\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::contract::DiagnosticCode;

    fn article_html() -> &'static str {
        r#"<!doctype html>
<html>
<head><title>Retry Patterns</title></head>
<body>
<nav><a href="/">Home</a><a href="/blog">Blog</a><a href="/about">About</a></nav>
<article>
<h1>Retry Patterns</h1>
<p>Exponential backoff retries a failed request after a delay that doubles each time, up to a configured maximum. That keeps a struggling dependency from being stampeded by a herd of identical clients.</p>
<p>Jitter spreads those retries so many clients do not wake up on the same tick. Together with a retry budget, the service stays available for new work instead of collapsing under correlated load.</p>
</article>
<footer>Copyright 2026</footer>
</body>
</html>"#
    }

    #[test]
    fn article_page_becomes_markdown_without_navigation_chrome() {
        let result = extract_html_file(article_html()).unwrap();
        assert!(result.text.contains("Exponential backoff"));
        assert!(result.text.contains("Retry Patterns"));
        assert!(!result.text.to_ascii_lowercase().contains("copyright 2026"));
        assert_eq!(result.document.title.as_deref(), Some("Retry Patterns"));
        assert_eq!(result.provenance.extractor, WEB_EXTRACTOR);
        assert_eq!(result.provenance.extractor_version, WEB_EXTRACTOR_VERSION);
        assert_eq!(result.document.format, SourceFormat::Html);
        assert!(result.stats.web.expect("web stats").readability_succeeded);
        assert!(!result
            .diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::WebSpaShellDetected));
    }

    #[test]
    fn readability_prefers_article_classes_over_longer_comment_chrome() {
        let html = r#"<!doctype html>
<html>
<head><title>Retry Patterns</title></head>
<body>
<div class="comments">
<p>This comment thread is longer than the article on purpose. Readers argue about timeouts, circuit breakers, and whether jitter belongs in the client or the library. None of that discussion is the source document CoWiki should keep.</p>
<p>A second comment repeats the same debate with more words about dashboards, paging, and who got paged last Friday. The old density heuristic would treat this pile of paragraphs as the winner.</p>
</div>
<div class="post-content entry-content">
<h1>Retry Patterns</h1>
<p>Exponential backoff retries a failed request after a delay that doubles each time, up to a configured maximum. That keeps a struggling dependency from being stampeded by a herd of identical clients.</p>
<p>Jitter spreads those retries so many clients do not wake up on the same tick. Together with a retry budget, the service stays available for new work instead of collapsing under correlated load.</p>
</div>
</body>
</html>"#;
        let result = extract_html_file(html).unwrap();
        assert!(result.text.contains("Exponential backoff"));
        assert!(!result.text.contains("comment thread"));
        assert!(!result.text.contains("who got paged"));
        assert_eq!(result.provenance.extractor, WEB_EXTRACTOR);
    }

    #[test]
    fn spa_shell_is_not_saved_as_an_article() {
        let html = r#"<html><body><div id="root"></div>
        <script>window.__NEXT_DATA__={"props":{}}</script>
        <noscript>You need to enable JavaScript to run this app.</noscript>
        </body></html>"#;
        let error = extract_html_file(html).unwrap_err();
        assert!(error.contains("dynamic capture"), "{error}");
    }

    #[test]
    fn login_form_is_rejected() {
        let html = r#"<html><head><title>Sign in</title></head>
        <body><h1>Sign in</h1><form><input type="password" name="password"></form></body></html>"#;
        let error = extract_html_file(html).unwrap_err();
        assert!(error.contains("login"), "{error}");
    }
}
