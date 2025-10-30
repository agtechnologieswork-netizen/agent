use anyhow::Result;
use clap::Parser;
use edda_integrations::{
    FetchSpreadsheetDataRequest, GetSpreadsheetMetadataRequest, GoogleSheetsClient,
    ReadRangeRequest, ToolResultDisplay,
};

#[derive(Parser)]
#[command(name = "google-sheets-example")]
#[command(about = "Google Sheets integration example")]
struct Args {
    /// Google Sheets URL or spreadsheet ID
    #[arg(short, long)]
    url: String,

    /// Optional range to read (e.g., "Sheet1!A1:C10")
    #[arg(short, long)]
    range: Option<String>,

    /// Show only metadata (don't fetch full data)
    #[arg(short, long)]
    metadata_only: bool,

    /// Output format: table or markdown
    #[arg(short, long, default_value = "table")]
    format: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    println!("=== Google Sheets Integration Example ===");
    println!("Spreadsheet: {}", args.url);
    println!(
        "Note: Credentials will be read from ~/.config/gspread/credentials.json or GOOGLE_SERVICE_ACCOUNT_KEY env var"
    );

    let client = GoogleSheetsClient::new().await?;

    // Get spreadsheet metadata
    println!("\n=== Spreadsheet Metadata ===");
    let request = GetSpreadsheetMetadataRequest {
        url_or_id: args.url.clone(),
    };
    match client.get_spreadsheet_metadata(&request).await {
        Ok(metadata) => {
            println!("{}", metadata.display());
        }
        Err(e) => println!("Error fetching metadata: {}", e),
    }

    // If only metadata requested, exit early
    if args.metadata_only {
        println!("\n=== Metadata-only mode completed ===");
        return Ok(());
    }

    // Handle specific range request
    if let Some(range) = &args.range {
        println!("\n=== Reading Specific Range: {} ===", range);
        let request = ReadRangeRequest {
            url_or_id: args.url.clone(),
            range: range.clone(),
        };
        match client.read_range(&request).await {
            Ok(result) => {
                if args.format == "markdown" {
                    println!("\n{}", result.display());
                } else {
                    println!("{}", result.display());
                }
            }
            Err(e) => println!("Error reading range '{}': {}", range, e),
        }
    } else {
        // Fetch full spreadsheet data
        println!("\n=== Full Spreadsheet Data ===");
        let request = FetchSpreadsheetDataRequest {
            url_or_id: args.url.clone(),
        };
        match client.fetch_spreadsheet_data(&request).await {
            Ok(data) => {
                if args.format == "markdown" {
                    println!("\n{}", client.to_markdown(&data));
                } else {
                    println!("{}", data.display());
                }
            }
            Err(e) => println!("Error fetching spreadsheet data: {}", e),
        }
    }

    println!("\n=== Integration example completed ===");
    Ok(())
}
