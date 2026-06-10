use lopdf::Document;
use std::collections::HashSet;
use std::io::Cursor;
use winnow::{
    ascii::{float, space0, space1},
    combinator::{eof, opt},
    token::take_while,
    Parser,
};

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

pub fn is_pdf_encrypted(data: &[u8]) -> bool {
    Document::load_from(Cursor::new(data))
        .map(|doc| doc.is_encrypted())
        .unwrap_or(false)
}

pub fn parse_pdf_bytes(data: &[u8]) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    let doc = Document::load_from(Cursor::new(data))?;
    parse_doc(&doc)
}

pub fn parse_pdf_bytes_with_password(
    data: &[u8],
    password: &str,
) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    let doc = Document::load_mem_with_options(data, lopdf::LoadOptions::with_password(password))?;
    parse_doc(&doc)
}

pub fn extract_lines_from_pdf(data: &[u8]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = Document::load_from(Cursor::new(data))?;
    let mut all_lines = Vec::new();

    for page_id in doc.page_iter() {
        let content_data = doc.get_page_content(page_id)?;
        let page_lines = extract_lines(&content_data);
        all_lines.extend(page_lines);
    }

    Ok(all_lines)
}

fn parse_doc(doc: &Document) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
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

        if b[i] == b'[' {
            let j = match s[i..].find(']') {
                Some(pos) => i + pos + 1,
                None => {
                    i += 1;
                    continue;
                }
            };
            let after = s[j..].trim_start();
            if after.starts_with("TJ") {
                let array_content = &s[i + 1..j - 1];
                let mut array_text = String::new();
                let mut ci = 0;
                let arr_bytes = array_content.as_bytes();
                while ci < arr_bytes.len() {
                    if arr_bytes[ci] == b'(' {
                        let mut depth = 1i32;
                        let mut cj = ci + 1;
                        while cj < arr_bytes.len() && depth > 0 {
                            if arr_bytes[cj] == b'\\' && cj + 1 < arr_bytes.len() {
                                cj += 2;
                                continue;
                            }
                            if arr_bytes[cj] == b'(' {
                                depth += 1;
                            } else if arr_bytes[cj] == b')' {
                                depth -= 1;
                            }
                            cj += 1;
                        }
                        let inner = &array_content[ci + 1..cj - 1];
                        let text = inner.replace("\\(", "(").replace("\\)", ")").replace("\\\\", "\\");
                        array_text.push_str(text.trim());
                        ci = cj;
                    } else {
                        ci += 1;
                    }
                }
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(array_text.trim());
            }
            i = j;
            continue;
        }

        if b[i].is_ascii_digit() || b[i] == b'-' || b[i] == b'.' {
            let rest = &s[i..];
            if let Some((_x, y, len)) = parse_td_coords(rest) {
                if let Some(prev) = last_y {
                    if (y - prev).abs() > 3.0 && !current.trim().is_empty() {
                        lines.push(current.trim().to_string());
                        current = String::new();
                    }
                }
                last_y = Some(y);
                i += len;
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

fn parse_td_coords(s: &str) -> Option<(f64, f64, usize)> {
    let mut input = s;
    let (x, _, y): (f64, _, f64) = (
        float::<_, f64, winnow::error::ContextError>,
        space1::<_, winnow::error::ContextError>,
        float::<_, f64, winnow::error::ContextError>,
    )
        .parse_next(&mut input)
        .ok()?;
    let _ = space0::<_, winnow::error::ContextError>.parse_next(&mut input).ok()?;
    if input.starts_with("Td") {
        Some((x, y, s.len() - input.len() + 2))
    } else {
        None
    }
}

fn find_date(text: &str) -> Option<(usize, String)> {
    for (i, _) in text.char_indices() {
        let mut rest = &text[i..];
        let before = rest;
        let result = (
            take_while(2..=2, |c: char| c.is_ascii_digit()),
            '/',
            take_while(2..=2, |c: char| c.is_ascii_digit()),
            '/',
            take_while(4..=4, |c: char| c.is_ascii_digit()),
            space0::<_, winnow::error::ContextError>,
            '|',
            space0::<_, winnow::error::ContextError>,
            take_while(2..=2, |c: char| c.is_ascii_digit()),
            ':',
            take_while(2..=2, |c: char| c.is_ascii_digit()),
        )
            .parse_next(&mut rest);
        if result.is_ok() {
            let end = i + (before.len() - rest.len());
            return Some((end, text[i..end].to_string()));
        }
    }
    None
}

fn has_date(text: &str) -> bool {
    find_date(text).is_some()
}

fn is_amount_word(s: &str) -> bool {
    let mut input = s;
    let result = (
        opt('+'),
        space0::<_, winnow::error::ContextError>,
        take_while(1.., |c: char| c.is_ascii_digit() || c == ','),
        opt(('.', take_while(1.., |c: char| c.is_ascii_digit()))),
        eof,
    )
        .parse_next(&mut input);
    result.is_ok()
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
    let mut transactions = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];

        if let Some((dm_end, date_text)) = find_date(line) {
            let date = date_text.replace(" ", "");
            let rest = line[dm_end..].trim().to_string();

            let mut desc_words = Vec::new();
            let mut amount = String::new();
            let mut is_credit = false;

            collect_words(&rest, &mut desc_words, &mut amount, &mut is_credit);

            let mut j = i + 1;
            while j < lines.len() && j < i + 8 {
                let next = &lines[j];

                if has_date(next)
                    || (next.contains("Domestic Transactions") && desc_words.is_empty())
                    || (next.contains("International Transactions") && desc_words.is_empty())
                    || is_stop_line(next)
                {
                    break;
                }

                collect_words(next, &mut desc_words, &mut amount, &mut is_credit);

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
    desc: &mut Vec<String>,
    amount: &mut String,
    is_credit: &mut bool,
) {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut w = 0;
    while w < words.len() {
        match words[w] {
            "C" => {
                if w + 1 < words.len() && is_amount_word(words[w + 1]) {
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
                if is_amount_word(word) && amount.is_empty() {
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
