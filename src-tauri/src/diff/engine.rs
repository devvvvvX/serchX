use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffHunk {
    pub tag: String, // "added", "removed", "equal"
    pub content: String,
    pub line_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffSummary {
    pub added_lines: usize,
    pub removed_lines: usize,
    pub unchanged_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionDiff {
    pub version_a_id: String,
    pub version_b_id: String,
    pub hunks: Vec<DiffHunk>,
    pub added_terms: HashSet<String>,
    pub removed_terms: HashSet<String>,
    pub summary: DiffSummary,
    pub is_truncated: bool,
}

pub struct DiffEngine {
    max_text_bytes: usize,
}

impl Default for DiffEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffEngine {
    pub fn new() -> Self {
        Self {
            max_text_bytes: 500 * 1024, // 500 KB default safety guard
        }
    }

    pub fn with_max_bytes(max_bytes: usize) -> Self {
        Self {
            max_text_bytes: max_bytes,
        }
    }

    pub fn normalize_text(&self, raw: &str) -> String {
        let mut normalized = String::with_capacity(raw.len());
        for line in raw.lines() {
            // Preserve leading indentation (spaces and tabs) so formatting changes are visible
            let first_non_ws = line
                .char_indices()
                .find(|(_, c)| *c != ' ' && *c != '\t')
                .map(|(i, _)| i);

            match first_non_ws {
                Some(idx) => {
                    let leading = &line[..idx];
                    normalized.push_str(leading);

                    let trimmed_rest = line[idx..].trim_end();
                    let mut in_space = false;
                    for ch in trimmed_rest.chars() {
                        if ch == ' ' || ch == '\t' {
                            if !in_space {
                                normalized.push(' ');
                                in_space = true;
                            }
                        } else {
                            normalized.push(ch);
                            in_space = false;
                        }
                    }
                }
                None => {
                    // Blank or whitespace-only line
                }
            }
            normalized.push('\n');
        }
        normalized
    }

    pub fn extract_terms(&self, text: &str, target: &mut HashSet<String>) {
        for word in text.split(|c: char| !c.is_alphanumeric()) {
            let lower = word.trim().to_lowercase();
            if lower.len() >= 2 {
                target.insert(lower);
            }
        }
    }

    pub fn compute_diff(
        &self,
        old_text: &str,
        new_text: &str,
        version_a_id: &str,
        version_b_id: &str,
    ) -> VersionDiff {
        let mut is_truncated = false;

        let old_slice = if old_text.len() > self.max_text_bytes {
            is_truncated = true;
            &old_text[..self.max_text_bytes]
        } else {
            old_text
        };

        let new_slice = if new_text.len() > self.max_text_bytes {
            is_truncated = true;
            &new_text[..self.max_text_bytes]
        } else {
            new_text
        };

        let old_clean = self.normalize_text(old_slice);
        let new_clean = self.normalize_text(new_slice);

        let old_lines: Vec<&str> = old_clean.lines().collect();
        let new_lines: Vec<&str> = new_clean.lines().collect();

        let raw_hunks = myers_diff(&old_lines, &new_lines);

        let mut hunks = Vec::with_capacity(raw_hunks.len());
        let mut added_terms = HashSet::new();
        let mut removed_terms = HashSet::new();
        let mut added_lines = 0;
        let mut removed_lines = 0;
        let mut unchanged_lines = 0;
        let mut new_line_idx = 1;

        for (tag, line_content) in raw_hunks {
            let formatted_content = line_content.to_string();

            match tag.as_str() {
                "removed" => {
                    removed_lines += 1;
                    self.extract_terms(&line_content, &mut removed_terms);
                    hunks.push(DiffHunk {
                        tag,
                        content: formatted_content,
                        line_number: None,
                    });
                }
                "added" => {
                    added_lines += 1;
                    self.extract_terms(&line_content, &mut added_terms);
                    hunks.push(DiffHunk {
                        tag,
                        content: formatted_content,
                        line_number: Some(new_line_idx),
                    });
                    new_line_idx += 1;
                }
                _ => {
                    unchanged_lines += 1;
                    hunks.push(DiffHunk {
                        tag,
                        content: formatted_content,
                        line_number: Some(new_line_idx),
                    });
                    new_line_idx += 1;
                }
            }
        }

        VersionDiff {
            version_a_id: version_a_id.to_string(),
            version_b_id: version_b_id.to_string(),
            hunks,
            added_terms,
            removed_terms,
            summary: DiffSummary {
                added_lines,
                removed_lines,
                unchanged_lines,
            },
            is_truncated,
        }
    }
}

/// Myers difference algorithm between two slices of strings
fn myers_diff(a: &[&str], b: &[&str]) -> Vec<(String, String)> {
    let n = a.len();
    let m = b.len();

    if n == 0 && m == 0 {
        return Vec::new();
    }
    if n == 0 {
        return b
            .iter()
            .map(|&s| ("added".to_string(), s.to_string()))
            .collect();
    }
    if m == 0 {
        return a
            .iter()
            .map(|&s| ("removed".to_string(), s.to_string()))
            .collect();
    }

    let max = n + m;
    let v_size = 2 * max + 1;
    let mut v = vec![0isize; v_size];
    let offset = max as isize;

    let mut trace: Vec<Vec<isize>> = Vec::new();

    let mut found = false;
    for d in 0..=max {
        trace.push(v.clone());
        for k in (-(d as isize)..=(d as isize)).step_by(2) {
            let mut x = if k == -(d as isize)
                || (k != (d as isize)
                    && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
            {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };

            let mut y = x - k;

            while (x as usize) < n && (y as usize) < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }

            v[(k + offset) as usize] = x;

            if (x as usize) >= n && (y as usize) >= m {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }

    // Backtrack to assemble edit path
    let mut x = n as isize;
    let mut y = m as isize;
    let mut edits = Vec::new();

    for d in (1..trace.len()).rev() {
        let v_prev = &trace[d];
        let k = x - y;

        let prev_k = if k == -(d as isize)
            || (k != (d as isize)
                && v_prev[(k - 1 + offset) as usize] < v_prev[(k + 1 + offset) as usize])
        {
            k + 1
        } else {
            k - 1
        };

        let prev_x = v_prev[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;

        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            edits.push(("equal".to_string(), a[x as usize].to_string()));
        }

        if d > 0 {
            if x == prev_x {
                y -= 1;
                edits.push(("added".to_string(), b[y as usize].to_string()));
            } else if y == prev_y {
                x -= 1;
                edits.push(("removed".to_string(), a[x as usize].to_string()));
            }
        }
    }

    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        edits.push(("equal".to_string(), a[x as usize].to_string()));
    }
    while x > 0 {
        x -= 1;
        edits.push(("removed".to_string(), a[x as usize].to_string()));
    }
    while y > 0 {
        y -= 1;
        edits.push(("added".to_string(), b[y as usize].to_string()));
    }

    edits.reverse();
    edits
}
