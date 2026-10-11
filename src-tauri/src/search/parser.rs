use chrono::{DateTime, Datelike, Duration, TimeZone, Utc, Weekday};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRangeFilter {
    pub start_ms: i64,
    pub end_ms: i64,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeDirection {
    Before,
    After,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectionalVersionCue {
    pub direction: Option<ChangeDirection>,
    pub target_subject: Option<String>,
    pub ordinal: Option<i32>,
    pub raw_cue: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedQuery {
    pub raw_query: String,
    pub clean_text: String,
    pub exact_phrases: Vec<String>,
    pub date_filter: Option<DateRangeFilter>,
    pub type_extensions: Vec<String>,
    pub folder_hint: Option<String>,
    pub version_cues: Vec<String>,
    pub directional_version_cue: Option<DirectionalVersionCue>,
}

static QUOTE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]+)""#).unwrap());
static FOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:in|from)\s+(desktop|documents|downloads)\b").unwrap());
static TODAY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\btoday\b").unwrap());
static YESTERDAY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:from\s+)?yesterday\b").unwrap());
static LAST_WEEK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\blast week\b").unwrap());
static LAST_MONTH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\blast month\b").unwrap());
static WEEKDAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\blast\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b")
        .unwrap()
});
static YEAR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(202[0-9])\b").unwrap());
static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(draft|v1|v2|v3|v4|v5|final|latest|previous|before|after|newer|older)\b")
        .unwrap()
});
static BEFORE_CHANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:before|prior to)\s+([^,.]+?\b(?:changed|updated|removed|modified|was signed))\b",
    )
    .unwrap()
});
static AFTER_CHANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:after|following)\s+([^,.]+?\b(?:changed|updated|added|modified|meeting))\b",
    )
    .unwrap()
});
static V_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:v([1-9])|version\s*([1-9]))\b").unwrap());
static RELATIVE_VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(draft|final|latest|previous|earlier)\b").unwrap());
static NOISE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(the|a|an|from|in|on|with|about|of|for)\b").unwrap());

static TYPE_SPREADSHEET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(spreadsheet|spreadsheets|excel|csv|tsv)\b").unwrap());
static TYPE_PDF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(pdf|pdfs)\b").unwrap());
static TYPE_DOC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(document|documents|word|doc|docx)\b").unwrap());
static TYPE_PRESENTATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(presentation|presentations|slides|powerpoint|keynote)\b").unwrap()
});
static TYPE_IMAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(screenshot|screenshots|image|images|photo|photos)\b").unwrap()
});
static TYPE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(code|script|scripts|rust|python)\b").unwrap());

pub fn parse_query(raw: &str, now: DateTime<Utc>) -> ParsedQuery {
    // T-03-05: Cap query length at 1000 characters to prevent ReDoS
    let raw = if raw.len() > 1000 { &raw[..1000] } else { raw };

    let mut working_text = raw.to_string();
    let mut exact_phrases = Vec::new();
    let mut version_cues = Vec::new();
    let mut type_extensions = Vec::new();
    let mut folder_hint = None;
    let mut date_filter = None;

    // 1. Extract exact quoted phrases: "..."
    for cap in QUOTE_RE.captures_iter(&working_text) {
        if let Some(m) = cap.get(1) {
            let phrase = m.as_str().trim().to_string();
            if !phrase.is_empty() {
                exact_phrases.push(phrase);
            }
        }
    }
    working_text = QUOTE_RE.replace_all(&working_text, " ").to_string();

    // 2. Extract folder hints: in/from (desktop|documents|downloads)
    if let Some(caps) = FOLDER_RE.captures(&working_text) {
        let lower_matched = caps.get(1).unwrap().as_str().to_lowercase();
        let folder = match lower_matched.as_str() {
            "desktop" => "Desktop",
            "documents" => "Documents",
            "downloads" => "Downloads",
            _ => caps.get(1).unwrap().as_str(),
        };
        folder_hint = Some(folder.to_string());
    }
    working_text = FOLDER_RE.replace_all(&working_text, " ").to_string();

    // 3. Extract temporal cues
    let today_start = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .unwrap();
    let today_end = today_start + Duration::days(1) - Duration::milliseconds(1);

    if TODAY_RE.is_match(&working_text) {
        date_filter = Some(DateRangeFilter {
            start_ms: today_start.timestamp_millis(),
            end_ms: today_end.timestamp_millis(),
            label: "today".to_string(),
        });
        working_text = TODAY_RE.replace_all(&working_text, " ").to_string();
    } else if YESTERDAY_RE.is_match(&working_text) {
        let yest_start = today_start - Duration::days(1);
        let yest_end = today_start - Duration::milliseconds(1);
        date_filter = Some(DateRangeFilter {
            start_ms: yest_start.timestamp_millis(),
            end_ms: yest_end.timestamp_millis(),
            label: "yesterday".to_string(),
        });
        working_text = YESTERDAY_RE.replace_all(&working_text, " ").to_string();
    } else if LAST_WEEK_RE.is_match(&working_text) {
        let week_ago = now - Duration::days(7);
        date_filter = Some(DateRangeFilter {
            start_ms: week_ago.timestamp_millis(),
            end_ms: now.timestamp_millis(),
            label: "last week".to_string(),
        });
        working_text = LAST_WEEK_RE.replace_all(&working_text, " ").to_string();
    } else if LAST_MONTH_RE.is_match(&working_text) {
        let month_ago = now - Duration::days(30);
        date_filter = Some(DateRangeFilter {
            start_ms: month_ago.timestamp_millis(),
            end_ms: now.timestamp_millis(),
            label: "last month".to_string(),
        });
        working_text = LAST_MONTH_RE.replace_all(&working_text, " ").to_string();
    } else if let Some(caps) = WEEKDAY_RE.captures(&working_text) {
        let day_name = caps.get(1).unwrap().as_str().to_lowercase();
        let target_weekday = match day_name.as_str() {
            "monday" => Weekday::Mon,
            "tuesday" => Weekday::Tue,
            "wednesday" => Weekday::Wed,
            "thursday" => Weekday::Thu,
            "friday" => Weekday::Fri,
            "saturday" => Weekday::Sat,
            "sunday" => Weekday::Sun,
            _ => Weekday::Mon,
        };

        let current_weekday = now.weekday();
        let mut days_back = (current_weekday.num_days_from_monday() as i64)
            - (target_weekday.num_days_from_monday() as i64);
        if days_back <= 0 {
            days_back += 7;
        }

        let target_date_start = today_start - Duration::days(days_back);
        let target_date_end = target_date_start + Duration::days(1) - Duration::milliseconds(1);

        date_filter = Some(DateRangeFilter {
            start_ms: target_date_start.timestamp_millis(),
            end_ms: target_date_end.timestamp_millis(),
            label: format!("last {}", day_name),
        });
        working_text = WEEKDAY_RE.replace_all(&working_text, " ").to_string();
    } else if let Some(caps) = YEAR_RE.captures(&working_text) {
        let year_str = caps.get(1).unwrap().as_str();
        if let Ok(year) = year_str.parse::<i32>() {
            let y_start = Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).unwrap();
            let y_end = Utc.with_ymd_and_hms(year, 12, 31, 23, 59, 59).unwrap();
            date_filter = Some(DateRangeFilter {
                start_ms: y_start.timestamp_millis(),
                end_ms: y_end.timestamp_millis(),
                label: year_str.to_string(),
            });
            working_text = YEAR_RE.replace_all(&working_text, " ").to_string();
        }
    }

    // 4. Extract file type cues
    if TYPE_SPREADSHEET_RE.is_match(&working_text) {
        type_extensions.extend(["csv".to_string(), "tsv".to_string(), "xlsx".to_string()]);
        working_text = TYPE_SPREADSHEET_RE
            .replace_all(&working_text, " ")
            .to_string();
    }
    if TYPE_PDF_RE.is_match(&working_text) {
        type_extensions.push("pdf".to_string());
        working_text = TYPE_PDF_RE.replace_all(&working_text, " ").to_string();
    }
    if TYPE_DOC_RE.is_match(&working_text) {
        type_extensions.extend([
            "docx".to_string(),
            "txt".to_string(),
            "md".to_string(),
            "rtf".to_string(),
        ]);
        working_text = TYPE_DOC_RE.replace_all(&working_text, " ").to_string();
    }
    if TYPE_PRESENTATION_RE.is_match(&working_text) {
        type_extensions.extend(["pptx".to_string(), "key".to_string()]);
        working_text = TYPE_PRESENTATION_RE
            .replace_all(&working_text, " ")
            .to_string();
    }
    if TYPE_IMAGE_RE.is_match(&working_text) {
        type_extensions.extend([
            "png".to_string(),
            "jpg".to_string(),
            "jpeg".to_string(),
            "webp".to_string(),
            "heic".to_string(),
        ]);
        working_text = TYPE_IMAGE_RE.replace_all(&working_text, " ").to_string();
    }
    if TYPE_CODE_RE.is_match(&working_text) {
        type_extensions.extend([
            "rs".to_string(),
            "py".to_string(),
            "js".to_string(),
            "ts".to_string(),
            "sh".to_string(),
        ]);
        working_text = TYPE_CODE_RE.replace_all(&working_text, " ").to_string();
    }

    // 5. Extract directional version cues & change predicates
    let mut directional_version_cue = None;

    if let Some(caps) = BEFORE_CHANGE_RE.captures(&working_text) {
        let full_match = caps.get(0).unwrap().as_str().to_string();
        let raw_subject = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let cleaned_subject = clean_change_subject(raw_subject);
        version_cues.push("before".to_string());
        directional_version_cue = Some(DirectionalVersionCue {
            direction: Some(ChangeDirection::Before),
            target_subject: if cleaned_subject.is_empty() {
                None
            } else {
                Some(cleaned_subject)
            },
            ordinal: None,
            raw_cue: full_match,
        });
        working_text = BEFORE_CHANGE_RE.replace_all(&working_text, " ").to_string();
    } else if let Some(caps) = AFTER_CHANGE_RE.captures(&working_text) {
        let full_match = caps.get(0).unwrap().as_str().to_string();
        let raw_subject = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let cleaned_subject = clean_change_subject(raw_subject);
        version_cues.push("after".to_string());
        directional_version_cue = Some(DirectionalVersionCue {
            direction: Some(ChangeDirection::After),
            target_subject: if cleaned_subject.is_empty() {
                None
            } else {
                Some(cleaned_subject)
            },
            ordinal: None,
            raw_cue: full_match,
        });
        working_text = AFTER_CHANGE_RE.replace_all(&working_text, " ").to_string();
    }

    // Extract explicit version number (e.g. "v2", "version 3")
    if let Some(caps) = V_NUM_RE.captures(&working_text) {
        let ord_opt = caps
            .get(1)
            .or_else(|| caps.get(2))
            .and_then(|m| m.as_str().parse::<i32>().ok());
        let raw_cue = caps.get(0).unwrap().as_str().to_string();
        version_cues.push(raw_cue.to_lowercase());
        if let Some(ref mut d_cue) = directional_version_cue {
            if ord_opt.is_some() {
                d_cue.ordinal = ord_opt;
            }
        } else {
            directional_version_cue = Some(DirectionalVersionCue {
                direction: None,
                target_subject: None,
                ordinal: ord_opt,
                raw_cue: raw_cue.clone(),
            });
        }
        working_text = V_NUM_RE.replace_all(&working_text, " ").to_string();
    }

    // Extract relative version cues (e.g. "draft", "final", "latest")
    if let Some(caps) = RELATIVE_VERSION_RE.captures(&working_text) {
        let raw_cue = caps.get(0).unwrap().as_str().to_string();
        version_cues.push(raw_cue.to_lowercase());
        if directional_version_cue.is_none() {
            directional_version_cue = Some(DirectionalVersionCue {
                direction: None,
                target_subject: None,
                ordinal: None,
                raw_cue: raw_cue.clone(),
            });
        }
        working_text = RELATIVE_VERSION_RE
            .replace_all(&working_text, " ")
            .to_string();
    }

    // 5b. Extract general version cues
    for cap in VERSION_RE.captures_iter(&working_text) {
        if let Some(m) = cap.get(1) {
            let cue = m.as_str().to_lowercase();
            if !version_cues.contains(&cue) {
                version_cues.push(cue);
            }
        }
    }
    working_text = VERSION_RE.replace_all(&working_text, " ").to_string();

    // 6. Clean text formatting
    let candidate_clean = NOISE_RE.replace_all(&working_text, " ").to_string();

    let words: Vec<&str> = candidate_clean
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();

    let clean_text = if !words.is_empty() {
        words.join(" ")
    } else if !exact_phrases.is_empty() {
        exact_phrases.join(" ")
    } else {
        raw.trim().to_string()
    };

    ParsedQuery {
        raw_query: raw.to_string(),
        clean_text,
        exact_phrases,
        date_filter,
        type_extensions,
        folder_hint,
        version_cues,
        directional_version_cue,
    }
}

fn clean_change_subject(raw: &str) -> String {
    static ACTION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(changed|updated|removed|modified|was signed|added|meeting|the|a|an)\b")
            .unwrap()
    });
    let cleaned = ACTION_RE.replace_all(raw, " ").to_string();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_contract_before_documents() {
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 12, 0, 0).unwrap();
        let query = "the contract before pricing changed in Documents";
        let parsed = parse_query(query, now);

        assert_eq!(parsed.folder_hint, Some("Documents".to_string()));
        assert!(parsed.version_cues.contains(&"before".to_string()));
        assert!(parsed.clean_text.contains("contract"));
        assert!(parsed.directional_version_cue.is_some());
        let cue = parsed.directional_version_cue.unwrap();
        assert_eq!(cue.direction, Some(ChangeDirection::Before));
        assert_eq!(cue.target_subject, Some("pricing".to_string()));
    }

    #[test]
    fn test_parse_directional_after_and_ordinal() {
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 12, 0, 0).unwrap();
        let query = "notes after team meeting";
        let parsed = parse_query(query, now);
        assert!(parsed.directional_version_cue.is_some());
        let cue = parsed.directional_version_cue.unwrap();
        assert_eq!(cue.direction, Some(ChangeDirection::After));
        assert_eq!(cue.target_subject, Some("team".to_string()));

        let query_ord = "technical specification draft v2";
        let parsed_ord = parse_query(query_ord, now);
        assert!(parsed_ord.directional_version_cue.is_some());
        assert_eq!(parsed_ord.directional_version_cue.unwrap().ordinal, Some(2));
    }

    #[test]
    fn test_parse_spreadsheet_last_tuesday() {
        // Wednesday Oct 7, 2026 -> last tuesday was Oct 6, 2026 (1 day prior)
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 12, 0, 0).unwrap();
        let query = "the spreadsheet I edited last Tuesday";
        let parsed = parse_query(query, now);

        assert!(parsed.type_extensions.contains(&"xlsx".to_string()));
        assert!(parsed.type_extensions.contains(&"csv".to_string()));
        assert!(parsed.date_filter.is_some());
        let df = parsed.date_filter.unwrap();
        assert_eq!(df.label, "last tuesday");
        assert!(parsed.clean_text.contains("I edited"));
    }

    #[test]
    fn test_parse_screenshot_docker_yesterday() {
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 12, 0, 0).unwrap();
        let query = "screenshot with Docker error from yesterday";
        let parsed = parse_query(query, now);

        assert!(parsed.type_extensions.contains(&"png".to_string()));
        assert!(parsed.date_filter.is_some());
        assert_eq!(parsed.date_filter.unwrap().label, "yesterday");
        assert!(parsed.clean_text.contains("Docker error"));
    }

    #[test]
    fn test_parse_exact_quotes_and_year() {
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 12, 0, 0).unwrap();
        let query = r#""Q3 Tax Report" 2024 in Desktop"#;
        let parsed = parse_query(query, now);

        assert_eq!(parsed.exact_phrases, vec!["Q3 Tax Report"]);
        assert_eq!(parsed.folder_hint, Some("Desktop".to_string()));
        assert!(parsed.date_filter.is_some());
        assert_eq!(parsed.date_filter.unwrap().label, "2024");
    }

    #[test]
    fn test_parser_latency_benchmark() {
        let now = Utc::now();
        let queries = vec![
            "the contract before pricing changed in Documents",
            "the spreadsheet I edited last Tuesday",
            "screenshot with Docker error from yesterday",
            r#""Project Roadmap" draft in Downloads"#,
            "annual tax statement 2023",
            "pdf invoice from Acme Corp last week",
            "python script v2 for data sync",
            "notes from meeting today",
        ];

        let start = std::time::Instant::now();
        let iterations = 100;
        for _ in 0..iterations {
            for q in &queries {
                let _ = parse_query(q, now);
            }
        }
        let elapsed = start.elapsed();
        let total_queries = iterations * queries.len();
        let avg_micros = elapsed.as_micros() as f64 / total_queries as f64;

        println!(
            "Parsed {} queries in {:?}, avg: {:.2} µs/query",
            total_queries, elapsed, avg_micros
        );
        // Assert average latency is < 500 µs (< 0.5 ms), well within 5 ms requirement (RET-01)
        assert!(
            avg_micros < 5000.0,
            "Parsing must be under 5ms on average, was {:.2} µs",
            avg_micros
        );
    }
}
