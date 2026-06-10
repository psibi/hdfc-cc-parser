use clap::Parser;
use csv::Writer;
use lopdf::Document;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hdfc-cc-parser")]
#[command(about = "Parse HDFC credit card PDF statements and export transactions to CSV")]
struct Cli {
    #[arg(default_value = "pw.pdf")]
    input: PathBuf,
    #[arg(short = 'o', long, default_value = "transactions.csv")]
    output: PathBuf,
}

#[derive(Debug)]
struct Transaction {
    date: String,
    description: String,
    amount: String,
    is_credit: bool,
}

/// Extract text lines from PDF content stream by parsing Tj and Td operators.
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
                if b[j] == b'(' { depth += 1; }
                else if b[j] == b')' { depth -= 1; }
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
                    transactions.push(Transaction {
                        date,
                        description,
                        amount,
                        is_credit,
                    });
                }
            }
        } else {
            i += 1;
        }
    }

    let mut seen = HashSet::new();
    transactions.retain(|t| seen.insert((t.date.clone(), t.amount.clone())));

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
                        // Only set is_credit from amount prefix if not already detected via "+"
                        if raw.starts_with('+') {
                            *is_credit = true;
                        }
                        // Don't overwrite is_credit=false if it was already true
                        *amount = raw.trim_start_matches('+').trim().replace(',', "").to_string();
                    }
                    w += 2;
                    continue;
                }
                w += 1;
            }
            "+" => {
                // Could be credit indicator or reward points
                // If followed by C, it's a credit indicator
                // If followed by a small number (rewards), skip
                if w + 1 < words.len() {
                    if words[w + 1] == "C" {
                        *is_credit = true;
                        w += 1;
                        continue;
                    }
                    // If followed by a small integer, it's reward points - skip both
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
                    // Standalone number - only use as amount if it has commas (money format)
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let doc = Document::load(&cli.input)?;
    let mut all_lines = Vec::new();

    for page_id in doc.page_iter() {
        let content_data = doc.get_page_content(page_id)?;
        let page_lines = extract_lines(&content_data);
        all_lines.extend(page_lines);
    }

    let transactions = parse_transactions(&all_lines);

    let mut wtr = Writer::from_path(&cli.output)?;
    wtr.write_record(["Date", "Description", "Amount"])?;

    for t in &transactions {
        let amount_val: f64 = t.amount.parse().unwrap_or(0.0);
        let signed_amount = if t.is_credit { amount_val } else { -amount_val };
        wtr.write_record([&t.date, &t.description, &format!("{:.2}", signed_amount)])?;
    }

    wtr.flush()?;
    println!("Exported {} transactions to {}", transactions.len(), cli.output.display());

    Ok(())
}
