use clap::Parser;
use csv::Writer;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let data = std::fs::read(&cli.input)?;
    let transactions = hdfc_cc_parser::parse_pdf_bytes(&data)?;

    let mut wtr = Writer::from_path(&cli.output)?;
    wtr.write_record(["Date", "Description", "Amount"])?;

    for t in &transactions {
        wtr.write_record([&t.date, &t.description, &format!("{:.2}", t.amount)])?;
    }

    wtr.flush()?;
    println!(
        "Exported {} transactions to {}",
        transactions.len(),
        cli.output.display()
    );

    Ok(())
}
