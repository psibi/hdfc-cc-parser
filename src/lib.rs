use lopdf::Document;
use regex::Regex;
use std::collections::HashSet;
use std::io::Cursor;

#[cfg_attr(
    feature = "wasm",
    wasm_bindgen::prelude::wasm_bindgen(getter_with_clone)
)]
#[derive(Debug, Clone)]
pub struct Transaction {
    pub date: String,
    pub description: String,
    pub amount: f64,
}

impl Transaction {
    pub fn is_credit(&self) -> bool {
        self.amount > 0.0
    }
}

pub fn parse_pdf_bytes(data: &[u8]) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    let doc = Document::load_from(Cursor::new(data))?;
    let mut all_lines = Vec::new();

    for page_id in doc.page_iter() {
        let content_data = doc.get_page_content(page_id)?;
        let page_lines = extract_lines(&content_data);
        all_lines.extend(page_lines);
    }

    Ok(parse_transactions(&all_lines))
}

fn extract_lines(content: &[u8]) -> Vec<String> {
    let s = String::from_utf8_lossy(content);
    let b = s.as_bytes();
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut last_y: Option<f64> = None;
    let mut i = 0;

    let td_re = Regex::new(r"^([\d.-]+)\s+([\d.-]+)\s+Td").unwrap();

    while i < b.len() {
        if b[i] == b'(' {
            let start = i;
            let mut depth = 1i32;
            let mut j = i + 1;
            while j < b.len() && depth > 0 {
                if b[j] == b'\\' && j + 1 < b.len() {
                    j += 2;
                    continue;
                }
                if b[j] == b'(' {
                    depth += 1;
                } else if b[j] == b')' {
                    depth -= 1;
                }
                j += 1;
            }
            let after = s[j..].trim_start();
            if after.starts_with("Tj") {
                let inner = &s[start + 1..j - 1];
                let text = inner.replace("\\(", "(").replace("\\)", ")").replace("\\\\", "\\");
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(text.trim());
            }
            i = j;
            continue;
        }

        if b[i].is_ascii_digit() || b[i] == b'-' || b[i] == b'.' {
            let rest = &s[i..];
            if let Some(caps) = td_re.captures(rest) {
                let y: f64 = caps[2].parse().unwrap_or(0.0);
                if let Some(prev) = last_y {
                    if (y - prev).abs() > 3.0 && !current.trim().is_empty() {
                        lines.push(current.trim().to_string());
                        current = String::new();
                    }
                }
                last_y = Some(y);
                i += caps.get(0).unwrap().len();
                continue;
            }
        }

        i += 1;
    }

    if !current.trim().is_empty() {
        lines.push(current.trim().to_string());
    }

    lines
}

fn is_stop_line(line: &str) -> bool {
    line.contains("Page ")
        || line == "DATE & TIME"
        || line.contains("TRANSACTION DESCRIPTION")
        || line.contains("Purchase Indicator")
        || line.contains("Reward Points")
        || line.contains("Smart EMI")
        || line.contains("GST Summary")
        || line.contains("Important Information")
        || line.contains("Useful Links")
        || line.contains("TOTAL AMOUNT")
        || line.contains("HSN Code")
        || line.contains("Your Card Control")
        || line.contains("Credit Card No")
        || line.contains("PREVIOUS STATEMENT")
        || line.contains("PAYMENTS/CREDITS")
        || line.contains("TOTAL CREDIT")
        || line.contains("Past Dues")
        || line.contains("Infinia")
        || line.contains("Eligible for")
        || line.contains("CONVERT TO EMI")
}

fn parse_transactions(lines: &[String]) -> Vec<Transaction> {
    let date_re = Regex::new(r"(\d{2}/\d{2}/\d{4}\s*\|\s*\d{2}:\d{2})").unwrap();
    let amount_re = Regex::new(r"^[+]?\s*[\d,]+\.?\d+$").unwrap();

    let mut transactions = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];

        if let Some(dm) = date_re.find(line) {
            let date = dm.as_str().replace(" ", "");
            let rest = line[dm.end()..].trim().to_string();

            let mut desc_words = Vec::new();
            let mut amount = String::new();
            let mut is_credit = false;

            collect_words(&rest, &amount_re, &mut desc_words, &mut amount, &mut is_credit);

            let mut j = i + 1;
            while j < lines.len() && j < i + 8 {
                let next = &lines[j];

                if date_re.is_match(next)
                    || (next.contains("Domestic Transactions") && desc_words.is_empty())
                    || (next.contains("International Transactions") && desc_words.is_empty())
                    || is_stop_line(next)
                {
                    break;
                }

                collect_words(next, &amount_re, &mut desc_words, &mut amount, &mut is_credit);

                if !amount.is_empty() {
                    j += 1;
                    break;
                }
                j += 1;
            }

            i = j;

            if !amount.is_empty() {
                let description = desc_words.join(" ").trim().to_string();
                if !description.is_empty() {
                    let amount_val: f64 = amount.parse().unwrap_or(0.0);
                    let signed = if is_credit { amount_val } else { -amount_val };
                    transactions.push(Transaction {
                        date,
                        description,
                        amount: signed,
                    });
                }
            }
        } else {
            i += 1;
        }
    }

    let mut seen = HashSet::new();
    transactions.retain(|t| seen.insert((t.date.clone(), t.amount.to_string())));

    transactions
}

fn collect_words(
    text: &str,
    amount_re: &Regex,
    desc: &mut Vec<String>,
    amount: &mut String,
    is_credit: &mut bool,
) {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut w = 0;
    while w < words.len() {
        match words[w] {
            "C" => {
                if w + 1 < words.len() && amount_re.is_match(words[w + 1]) {
                    if amount.is_empty() {
                        let raw = words[w + 1];
                        if raw.starts_with('+') {
                            *is_credit = true;
                        }
                        *amount = raw.trim_start_matches('+').trim().replace(',', "").to_string();
                    }
                    w += 2;
                    continue;
                }
                w += 1;
            }
            "+" => {
                if w + 1 < words.len() {
                    if words[w + 1] == "C" {
                        *is_credit = true;
                        w += 1;
                        continue;
                    }
                    if let Ok(n) = words[w + 1].parse::<u32>() {
                        if n < 10000 {
                            w += 2;
                            continue;
                        }
                    }
                }
                *is_credit = true;
                w += 1;
            }
            "l" | "REWARDS" | "AMOUNT" | "PI" | "EMI" => {
                w += 1;
            }
            word => {
                if amount_re.is_match(word) && amount.is_empty() {
                    if word.contains(',') {
                        *amount = word.trim_start_matches('+').trim().replace(',', "").to_string();
                    }
                    w += 1;
                } else {
                    desc.push(word.to_string());
                    w += 1;
                }
            }
        }
    }
}
