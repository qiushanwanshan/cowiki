//! Web completeness and chrome detection. Static extract never treats a
//! login wall or SPA shell as a successful article.

use super::policy::{
    WEB_ARTICLE_TO_BODY_RATIO_LIMIT, WEB_LARGE_HTML_BYTES, WEB_LINK_TEXT_RATIO_LIMIT,
    WEB_MIN_ARTICLE_CHARS,
};
use crate::extract::contract::{
    DiagnosticCode, ExtractResult, QualityClass, QualityReason, SourceFormat,
};

pub fn collect(result: &ExtractResult, reasons: &mut Vec<QualityReason>) {
    if !matches!(
        result.document.format,
        SourceFormat::Html | SourceFormat::Url
    ) {
        return;
    }
    let Some(web) = result.stats.web.as_ref() else {
        return;
    };

    if web.login_or_challenge_detected {
        reasons.push(QualityReason::new(
            DiagnosticCode::WebLoginOrChallenge,
            QualityClass::HardFail,
            "page looks like a login form, captcha, or challenge wall",
        ));
        return;
    }

    if web.spa_shell_detected && web.article_text_chars < WEB_MIN_ARTICLE_CHARS {
        reasons.push(QualityReason::new(
            DiagnosticCode::WebSpaShellDetected,
            QualityClass::Completeness,
            "page looks like a JavaScript application shell with no static article",
        ));
        return;
    }

    if !web.readability_succeeded || web.article_text_chars < WEB_MIN_ARTICLE_CHARS {
        if web.response_bytes >= WEB_LARGE_HTML_BYTES || web.dom_visible_text_chars >= 500 {
            reasons.push(QualityReason::new(
                DiagnosticCode::WebReadabilityEmpty,
                QualityClass::Completeness,
                format!(
                    "article text is {} characters while the page has {} visible characters",
                    web.article_text_chars, web.dom_visible_text_chars
                ),
            ));
        }
    }

    if web.dom_visible_text_chars > 0
        && web.article_to_body_text_ratio < WEB_ARTICLE_TO_BODY_RATIO_LIMIT
        && web.article_text_chars < web.dom_visible_text_chars
        && web.dom_visible_text_chars >= 500
    {
        reasons.push(QualityReason::new(
            DiagnosticCode::WebReadabilityEmpty,
            QualityClass::Completeness,
            format!(
                "readability kept {:.0}% of visible page text",
                web.article_to_body_text_ratio * 100.0
            ),
        ));
    }

    if web.link_text_ratio > WEB_LINK_TEXT_RATIO_LIMIT {
        let class = if web.article_text_chars >= WEB_MIN_ARTICLE_CHARS {
            QualityClass::StructureWarn
        } else {
            QualityClass::Completeness
        };
        reasons.push(QualityReason::new(
            DiagnosticCode::WebNavigationDensityHigh,
            class,
            format!(
                "link text is {:.0}% of visible text",
                web.link_text_ratio * 100.0
            ),
        ));
    }
}
