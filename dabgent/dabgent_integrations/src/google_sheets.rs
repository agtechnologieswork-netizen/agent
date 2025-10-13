use anyhow::{Result, anyhow};
use google_sheets4::{Sheets, hyper_rustls, hyper_util};
use log::{debug, info, warn};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetSpreadsheetMetadataRequest {
    pub url_or_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadRangeRequest {
    pub url_or_id: String,
    pub range: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FetchSpreadsheetDataRequest {
    pub url_or_id: String,
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SpreadsheetData {
    pub title: String,
    pub spreadsheet_id: String,
    pub sheets: Vec<SheetData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SheetData {
    pub id: i32,
    pub title: String,
    pub values: Vec<Vec<String>>,
    pub formulas: Vec<Vec<String>>,
    pub row_count: i32,
    pub column_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpreadsheetMetadata {
    pub title: String,
    pub spreadsheet_id: String,
    pub sheet_count: usize,
    pub sheets: Vec<SheetMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SheetMetadata {
    pub id: i32,
    pub title: String,
    pub row_count: i32,
    pub column_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadRangeResult {
    pub values: Vec<Vec<String>>,
}

// ============================================================================
// Display Trait for Tool Results
// ============================================================================

use crate::ToolResultDisplay;

impl ToolResultDisplay for SpreadsheetMetadata {
    fn display(&self) -> String {
        let mut lines = vec![
            format!("Spreadsheet: {}", self.title),
            format!("ID: {}", self.spreadsheet_id),
            format!("Sheets: {}", self.sheet_count),
            String::new(),
        ];

        for sheet in &self.sheets {
            lines.push(format!(
                "• {} ({}x{} cells)",
                sheet.title, sheet.row_count, sheet.column_count
            ));
        }

        lines.join("\n")
    }
}

impl ToolResultDisplay for ReadRangeResult {
    fn display(&self) -> String {
        if self.values.is_empty() {
            "No data found in range.".to_string()
        } else {
            let mut lines = vec![
                format!("Found {} rows:", self.values.len()),
                String::new(),
            ];

            for (i, row) in self.values.iter().enumerate().take(100) {
                lines.push(format!("  Row {}: {}", i + 1, row.join(", ")));
            }

            if self.values.len() > 100 {
                lines.push(format!(
                    "\n... showing first 100 of {} total rows",
                    self.values.len()
                ));
            }

            lines.join("\n")
        }
    }
}

impl ToolResultDisplay for SpreadsheetData {
    fn display(&self) -> String {
        let mut lines = vec![
            format!("Spreadsheet: {}", self.title),
            format!("ID: {}", self.spreadsheet_id),
            format!("Sheets: {}", self.sheets.len()),
            String::new(),
        ];

        for sheet in &self.sheets {
            lines.push(format!(
                "Sheet: {} ({}x{})",
                sheet.title, sheet.row_count, sheet.column_count
            ));

            if !sheet.values.is_empty() {
                lines.push(format!("  Rows: {}", sheet.values.len()));

                // Show first few rows as sample
                for (i, row) in sheet.values.iter().enumerate().take(5) {
                    if !row.is_empty() {
                        lines.push(format!("    Row {}: {}", i + 1, row.join(", ")));
                    }
                }
                if sheet.values.len() > 5 {
                    lines.push("    ...".to_string());
                }
            }
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

pub struct GoogleSheetsClient {
    hub: Sheets<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>>,
}

impl GoogleSheetsClient {
    /// Create a new Google Sheets client
    ///
    /// Credentials are read from:
    /// 1. ~/.config/gspread/credentials.json (preferred, standard gspread location)
    /// 2. GOOGLE_SERVICE_ACCOUNT_KEY environment variable (fallback)
    ///
    /// To set up credentials:
    /// 1. Download your service account JSON file from Google Cloud Console
    /// 2. Either:
    ///    - Place it at ~/.config/gspread/credentials.json, or
    ///    - Set GOOGLE_SERVICE_ACCOUNT_KEY to the JSON content
    pub async fn new() -> Result<Self> {
        // Try to read from standard gspread location first, then fall back to environment variable
        let service_account_key = Self::read_credentials()?;

        // Parse the service account key
        let key: ServiceAccountKey = serde_json::from_str(&service_account_key)
            .map_err(|e| anyhow!("Failed to parse service account key: {}", e))?;

        // Create authenticator
        let auth = ServiceAccountAuthenticator::builder(key)
            .build()
            .await
            .map_err(|e| anyhow!("Failed to build authenticator: {}", e))?;

        // Create HTTPS connector
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .unwrap()
            .https_only()
            .enable_http1()
            .enable_http2()
            .build();

        // Create the Sheets hub
        let hub = Sheets::new(
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(connector),
            auth,
        );

        Ok(Self { hub })
    }

    fn read_credentials() -> Result<String> {
        // Try to read from ~/.config/gspread/credentials.json (standard gspread location)
        if let Some(home_dir) = std::env::var_os("HOME") {
            let mut credentials_path = PathBuf::from(home_dir);
            credentials_path.push(".config");
            credentials_path.push("gspread");
            credentials_path.push("credentials.json");

            if credentials_path.exists() {
                debug!("Reading Google credentials from: {:?}", credentials_path);
                let credentials_content =
                    std::fs::read_to_string(&credentials_path).map_err(|e| {
                        anyhow!(
                            "Failed to read credentials file at {:?}: {}",
                            credentials_path,
                            e
                        )
                    })?;
                return Ok(credentials_content);
            } else {
                debug!("Credentials file not found at: {:?}", credentials_path);
            }
        }

        // Fall back to environment variable
        debug!("Trying to read credentials from GOOGLE_SERVICE_ACCOUNT_KEY environment variable");
        std::env::var("GOOGLE_SERVICE_ACCOUNT_KEY").map_err(|_| {
            anyhow!(
                "Could not find Google credentials. Please either:\n\
                1. Place your service account JSON file at ~/.config/gspread/credentials.json, or\n\
                2. Set the GOOGLE_SERVICE_ACCOUNT_KEY environment variable with the JSON content"
            )
        })
    }

    /// Create the gspread credentials directory if it doesn't exist
    pub fn ensure_credentials_dir() -> Result<PathBuf> {
        if let Some(home_dir) = std::env::var_os("HOME") {
            let mut credentials_dir = PathBuf::from(home_dir);
            credentials_dir.push(".config");
            credentials_dir.push("gspread");

            if !credentials_dir.exists() {
                std::fs::create_dir_all(&credentials_dir).map_err(|e| {
                    anyhow!(
                        "Failed to create credentials directory at {:?}: {}",
                        credentials_dir,
                        e
                    )
                })?;
                info!("Created credentials directory at: {:?}", credentials_dir);
            }

            let mut credentials_path = credentials_dir;
            credentials_path.push("credentials.json");
            Ok(credentials_path)
        } else {
            Err(anyhow!("HOME environment variable not set"))
        }
    }

    fn extract_spreadsheet_id(url_or_id: &str) -> Result<String> {
        // If it's already a spreadsheet ID (no slashes), return as-is
        if !url_or_id.contains('/') {
            return Ok(url_or_id.to_string());
        }

        // Extract from Google Sheets URL
        let re = Regex::new(r"/spreadsheets/d/([a-zA-Z0-9-_]+)")
            .map_err(|e| anyhow!("Failed to compile regex: {}", e))?;

        if let Some(captures) = re.captures(url_or_id)
            && let Some(id) = captures.get(1) {
                return Ok(id.as_str().to_string());
            }

        Err(anyhow!(
            "Could not extract spreadsheet ID from: {}",
            url_or_id
        ))
    }

    pub async fn get_spreadsheet_metadata(
        &self,
        request: &GetSpreadsheetMetadataRequest,
    ) -> Result<SpreadsheetMetadata> {
        self.get_spreadsheet_metadata_impl(&request.url_or_id).await
    }

    async fn get_spreadsheet_metadata_impl(&self, url_or_id: &str) -> Result<SpreadsheetMetadata> {
        let spreadsheet_id = Self::extract_spreadsheet_id(url_or_id)?;
        debug!("Getting metadata for spreadsheet: {}", spreadsheet_id);

        let result = self
            .hub
            .spreadsheets()
            .get(&spreadsheet_id)
            .doit()
            .await
            .map_err(|e| anyhow!("Failed to get spreadsheet: {}", e))?;

        let spreadsheet = result.1;

        let sheets: Vec<SheetMetadata> = spreadsheet
            .sheets
            .unwrap_or_default()
            .into_iter()
            .map(|sheet| {
                let properties = sheet.properties.unwrap_or_default();
                let grid_properties = properties.grid_properties.unwrap_or_default();

                SheetMetadata {
                    id: properties.sheet_id.unwrap_or(0),
                    title: properties.title.unwrap_or_else(|| "Untitled".to_string()),
                    row_count: grid_properties.row_count.unwrap_or(0),
                    column_count: grid_properties.column_count.unwrap_or(0),
                }
            })
            .collect();

        let sheet_count = sheets.len();

        Ok(SpreadsheetMetadata {
            title: spreadsheet
                .properties
                .and_then(|p| p.title)
                .unwrap_or_else(|| "Untitled Spreadsheet".to_string()),
            spreadsheet_id,
            sheet_count,
            sheets,
        })
    }

    pub async fn read_range(&self, request: &ReadRangeRequest) -> Result<ReadRangeResult> {
        self.read_range_impl(&request.url_or_id, &request.range).await
    }

    async fn read_range_impl(&self, url_or_id: &str, range: &str) -> Result<ReadRangeResult> {
        let spreadsheet_id = Self::extract_spreadsheet_id(url_or_id)?;
        let values = self.read_range_internal(&spreadsheet_id, range).await?;
        Ok(ReadRangeResult { values })
    }

    async fn read_range_internal(&self, spreadsheet_id: &str, range: &str) -> Result<Vec<Vec<String>>> {
        debug!(
            "Reading range '{}' from spreadsheet: {}",
            range, spreadsheet_id
        );

        let result = self
            .hub
            .spreadsheets()
            .values_get(spreadsheet_id, range)
            .doit()
            .await
            .map_err(|e| anyhow!("Failed to read range: {}", e))?;

        let value_range = result.1;

        let values = value_range
            .values
            .unwrap_or_default()
            .into_iter()
            .map(|row| row.into_iter().map(|cell| cell.to_string()).collect())
            .collect();

        Ok(values)
    }

    pub async fn fetch_spreadsheet_data(
        &self,
        request: &FetchSpreadsheetDataRequest,
    ) -> Result<SpreadsheetData> {
        self.fetch_spreadsheet_data_impl(&request.url_or_id).await
    }

    async fn fetch_spreadsheet_data_impl(&self, url_or_id: &str) -> Result<SpreadsheetData> {
        let spreadsheet_id = Self::extract_spreadsheet_id(url_or_id)?;
        info!("Fetching full data for spreadsheet: {}", spreadsheet_id);

        // Get spreadsheet metadata first - but don't include grid data for large sheets
        // We'll fetch data per sheet using the values API instead
        let result = self
            .hub
            .spreadsheets()
            .get(&spreadsheet_id)
            .include_grid_data(false) // Changed to false to avoid timeouts
            .doit()
            .await
            .map_err(|e| anyhow!("Failed to get spreadsheet metadata: {}", e))?;

        let spreadsheet = result.1;

        let title = spreadsheet
            .properties
            .and_then(|p| p.title)
            .unwrap_or_else(|| "Untitled Spreadsheet".to_string());

        let mut sheets = Vec::new();

        for sheet in spreadsheet.sheets.unwrap_or_default() {
            let properties = sheet.properties.unwrap_or_default();
            let sheet_title = properties.title.unwrap_or_else(|| "Untitled".to_string());
            let sheet_id = properties.sheet_id.unwrap_or(0);

            let grid_properties = properties.grid_properties.unwrap_or_default();
            let row_count = grid_properties.row_count.unwrap_or(0);
            let column_count = grid_properties.column_count.unwrap_or(0);

            debug!(
                "Processing sheet '{}' ({}x{} cells)",
                sheet_title, row_count, column_count
            );

            // Fetch values using the values API (more reliable for large sheets)
            let values = if row_count > 0 && column_count > 0 {
                // Limit to reasonable range to avoid timeouts - use first 100 rows and 26 columns (A-Z)
                let max_rows = std::cmp::min(row_count, 100);
                let max_cols = std::cmp::min(column_count, 26);
                let col_letter = char::from(b'A' + (max_cols - 1) as u8);
                let range = format!("{}!A1:{}{}", sheet_title, col_letter, max_rows);

                debug!("Fetching range: {}", range);
                match self.read_range_internal(&spreadsheet_id, &range).await {
                    Ok(sheet_values) => {
                        debug!(
                            "Successfully fetched {} rows from sheet '{}'",
                            sheet_values.len(),
                            sheet_title
                        );
                        sheet_values
                    }
                    Err(e) => {
                        warn!("Failed to fetch values for sheet '{}': {}", sheet_title, e);
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            };

            // For this API, we don't get formulas - would need separate API call
            let formulas = Vec::new();

            sheets.push(SheetData {
                id: sheet_id,
                title: sheet_title,
                values,
                formulas,
                row_count,
                column_count,
            });
        }

        Ok(SpreadsheetData {
            title,
            spreadsheet_id,
            sheets,
        })
    }

    pub fn to_markdown(&self, data: &SpreadsheetData) -> String {
        let mut markdown = String::new();

        markdown.push_str(&format!("# {}\n\n", data.title));
        markdown.push_str(&format!(
            "**Spreadsheet ID:** `{}`\n\n",
            data.spreadsheet_id
        ));

        for sheet in &data.sheets {
            markdown.push_str(&format!("## {} (ID: {})\n\n", sheet.title, sheet.id));
            markdown.push_str(&format!(
                "**Dimensions:** {} rows × {} columns\n\n",
                sheet.row_count, sheet.column_count
            ));

            if !sheet.values.is_empty() {
                // Take first few rows for preview
                let preview_rows = sheet.values.iter().take(10);
                let max_cols = preview_rows.clone().map(|row| row.len()).max().unwrap_or(0);

                if max_cols > 0 {
                    // Header row
                    markdown.push('|');
                    for i in 0..max_cols.min(10) {
                        markdown.push_str(&format!(" Col {} |", i + 1));
                    }
                    markdown.push('\n');

                    // Separator row
                    markdown.push('|');
                    for _ in 0..max_cols.min(10) {
                        markdown.push_str("-------|");
                    }
                    markdown.push('\n');

                    // Data rows
                    for row in preview_rows {
                        markdown.push('|');
                        for i in 0..max_cols.min(10) {
                            let empty_string = String::new();
                            let cell = row.get(i).unwrap_or(&empty_string);
                            let escaped = cell.replace('|', "\\|").replace('\n', " ");
                            markdown.push_str(&format!(" {} |", escaped));
                        }
                        markdown.push('\n');
                    }

                    if sheet.values.len() > 10 {
                        markdown.push_str(&format!(
                            "\n*... and {} more rows*\n",
                            sheet.values.len() - 10
                        ));
                    }
                }
            } else {
                markdown.push_str("*No data available*\n");
            }

            markdown.push('\n');
        }

        markdown
    }
}
