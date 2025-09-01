use anyhow::Result;
use clap::Parser;
use integrations::GoogleSheetsClient;

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
    
    let client = GoogleSheetsClient::new().await?;
    
    // Get spreadsheet metadata
    println!("\n=== Spreadsheet Metadata ===");
    match client.get_spreadsheet_metadata(&args.url).await {
        Ok(metadata) => {
            println!("Title: {}", metadata.title);
            println!("Number of sheets: {}", metadata.sheet_count);
            
            println!("\nSheets:");
            for sheet in &metadata.sheets {
                println!("  - {} (ID: {}, {}x{} cells)", 
                    sheet.title, sheet.id, sheet.row_count, sheet.column_count);
            }
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
        match client.read_range(&args.url, range).await {
            Ok(values) => {
                println!("Range '{}' contains {} rows", range, values.len());
                
                if args.format == "markdown" {
                    println!("\n| Row | Values |");
                    println!("|-----|--------|");
                    for (i, row) in values.iter().enumerate() {
                        println!("| {} | {:?} |", i + 1, row);
                    }
                } else {
                    for (i, row) in values.iter().enumerate() {
                        println!("  Row {}: {:?}", i + 1, row);
                    }
                }
            }
            Err(e) => println!("Error reading range '{}': {}", range, e),
        }
    } else {
        // Fetch full spreadsheet data
        println!("\n=== Full Spreadsheet Data ===");
        match client.fetch_spreadsheet_data(&args.url).await {
            Ok(data) => {
                println!("Spreadsheet: {}", data.title);
                println!("Number of sheets: {}", data.sheets.len());
                
                if args.format == "markdown" {
                    println!("\n{}", client.to_markdown(&data));
                } else {
                    for sheet in &data.sheets {
                        println!("\nSheet: {} (ID: {})", sheet.title, sheet.id);
                        println!("Values rows: {}, Formulas rows: {}", 
                            sheet.values.len(), sheet.formulas.len());
                        
                        // Show first few rows as sample
                        for (i, row) in sheet.values.iter().take(5).enumerate() {
                            println!("  Row {}: {:?}", i + 1, row);
                        }
                        
                        if sheet.values.len() > 5 {
                            println!("  ... and {} more rows", sheet.values.len() - 5);
                        }
                    }
                }
            }
            Err(e) => println!("Error fetching spreadsheet data: {}", e),
        }
    }
    
    println!("\n=== Integration example completed ===");
    Ok(())
}