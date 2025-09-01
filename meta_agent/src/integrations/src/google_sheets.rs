use std::path::Path;
use anyhow::{Result, anyhow};
use log::{info, debug};
use google_sheets4::{Sheets, hyper_rustls};
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};
use hyper_util::client::legacy::{connect::HttpConnector, Client};

#[derive(Debug)]
pub struct SpreadsheetData {
    pub title: String,
    pub sheets: Vec<SheetData>,
}

#[derive(Debug)]
pub struct SheetData {
    pub title: String,
    pub id: i32,
    pub values: Vec<Vec<String>>,
    pub formulas: Vec<Vec<String>>,
}

#[derive(Debug)]
pub struct SpreadsheetMetadata {
    pub title: String,
    pub sheet_count: usize,
    pub sheets: Vec<SheetMetadata>,
}

#[derive(Debug)]
pub struct SheetMetadata {
    pub title: String,
    pub id: i32,
    pub row_count: i32,
    pub column_count: i32,
}

pub struct GoogleSheetsClient {
    sheets: Sheets<hyper_rustls::HttpsConnector<HttpConnector>>,
}

impl GoogleSheetsClient {
    pub async fn new() -> Result<Self> {
        let authenticator = Self::create_authenticator().await?;
        
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| anyhow!("Failed to create HTTPS connector: {}", e))?
            .https_or_http()
            .enable_http1()
            .build();
        let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(connector);
        
        let sheets = Sheets::new(client, authenticator);
        
        Ok(Self { sheets })
    }

    async fn create_authenticator() -> Result<yup_oauth2::authenticator::Authenticator<hyper_rustls::HttpsConnector<HttpConnector>>> {
        // Try environment variable first
        if let Ok(creds_json) = std::env::var("GOOGLE_SHEETS_CREDENTIALS") {
            debug!("Using service account credentials from GOOGLE_SHEETS_CREDENTIALS env var");
            match serde_json::from_str::<ServiceAccountKey>(&creds_json) {
                Ok(key) => {
                    return ServiceAccountAuthenticator::builder(key)
                        .build()
                        .await
                        .map_err(|e| anyhow!("Failed to create authenticator from env var: {}", e));
                }
                Err(e) => {
                    debug!("Failed to parse GOOGLE_SHEETS_CREDENTIALS: {}", e);
                }
            }
        }

        // Try service account paths
        let service_account_paths = [
            std::env::var("HOME").map(|h| format!("{}/.config/gspread/service_account.json", h)).unwrap_or_default(),
            std::env::var("HOME").map(|h| format!("{}/.config/gspread/credentials.json", h)).unwrap_or_default(),
        ];

        for path_str in &service_account_paths {
            if path_str.is_empty() {
                continue;
            }
            
            let path = Path::new(path_str);
            if path.exists() {
                debug!("Trying service account file: {}", path_str);
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        match serde_json::from_str::<ServiceAccountKey>(&content) {
                            Ok(key) => {
                                info!("Authenticated with service account from {}", path_str);
                                return ServiceAccountAuthenticator::builder(key)
                                    .build()
                                    .await
                                    .map_err(|e| anyhow!("Failed to create authenticator from {}: {}", path_str, e));
                            }
                            Err(e) => {
                                debug!("Failed to parse service account file {}: {}", path_str, e);
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to read service account file {}: {}", path_str, e);
                    }
                }
            }
        }

        Err(anyhow!(
            "No valid Google Sheets credentials found. Please set up one of:\n\
             1. GOOGLE_SHEETS_CREDENTIALS environment variable with service account JSON\n\
             2. Service account JSON at ~/.config/gspread/service_account.json\n\
             3. Service account JSON at ~/.config/gspread/credentials.json"
        ))
    }

    fn extract_spreadsheet_id(&self, url_or_id: &str) -> Result<String> {
        // If it's a URL, extract the ID
        if url_or_id.contains("/spreadsheets/d/") {
            use regex::Regex;
            let re = Regex::new(r"/spreadsheets/d/([a-zA-Z0-9-_]+)").unwrap();
            if let Some(caps) = re.captures(url_or_id) {
                return Ok(caps[1].to_string());
            }
        }
        
        // If it's already an ID, return as is
        if url_or_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Ok(url_or_id.to_string());
        }
        
        Err(anyhow!("Could not extract spreadsheet ID from: {}", url_or_id))
    }

    pub async fn get_spreadsheet_metadata(&self, spreadsheet_url_or_id: &str) -> Result<SpreadsheetMetadata> {
        let spreadsheet_id = self.extract_spreadsheet_id(spreadsheet_url_or_id)?;
        
        debug!("Getting metadata for spreadsheet: {}", spreadsheet_id);
        
        let result = self.sheets
            .spreadsheets()
            .get(&spreadsheet_id)
            .doit()
            .await
            .map_err(|e| anyhow!("Failed to get spreadsheet metadata: {}", e))?;
        
        let spreadsheet = result.1;
        
        let title = spreadsheet.properties
            .as_ref()
            .and_then(|p| p.title.as_ref())
            .unwrap_or(&"Unknown".to_string())
            .to_string();
        
        let sheets: Vec<_> = spreadsheet.sheets
            .unwrap_or_default()
            .into_iter()
            .map(|sheet| {
                let properties = sheet.properties.unwrap_or_default();
                SheetMetadata {
                    title: properties.title.unwrap_or_else(|| "Unknown".to_string()),
                    id: properties.sheet_id.unwrap_or(0),
                    row_count: properties.grid_properties
                        .as_ref()
                        .and_then(|g| g.row_count)
                        .unwrap_or(0),
                    column_count: properties.grid_properties
                        .as_ref()
                        .and_then(|g| g.column_count)
                        .unwrap_or(0),
                }
            })
            .collect();
        
        let sheet_count = sheets.len();
        
        Ok(SpreadsheetMetadata {
            title,
            sheet_count,
            sheets,
        })
    }

    pub async fn fetch_spreadsheet_data(&self, spreadsheet_url_or_id: &str) -> Result<SpreadsheetData> {
        let spreadsheet_id = self.extract_spreadsheet_id(spreadsheet_url_or_id)?;
        
        info!("Fetching full spreadsheet data: {}", spreadsheet_id);
        
        let metadata = self.get_spreadsheet_metadata(&spreadsheet_id).await?;
        
        let mut sheets_data = Vec::new();
        
        for sheet_meta in &metadata.sheets {
            debug!("Fetching data for sheet: {}", sheet_meta.title);
            
            // Get values
            let values_range = format!("{}!A1:ZZ", sheet_meta.title);
            let values_result = self.sheets
                .spreadsheets()
                .values_get(&spreadsheet_id, &values_range)
                .doit()
                .await;
            
            let values = match values_result {
                Ok((_, value_range)) => {
                    value_range.values.unwrap_or_default()
                        .into_iter()
                        .map(|row| {
                            row.into_iter()
                                .map(|cell| cell.as_str().unwrap_or("").to_string())
                                .collect()
                        })
                        .collect()
                }
                Err(e) => {
                    debug!("Failed to get values for sheet {}: {}", sheet_meta.title, e);
                    Vec::new()
                }
            };
            
            // Get formulas
            let formulas_result = self.sheets
                .spreadsheets()
                .values_get(&spreadsheet_id, &values_range)
                .value_render_option("FORMULA")
                .doit()
                .await;
            
            let formulas = match formulas_result {
                Ok((_, value_range)) => {
                    value_range.values.unwrap_or_default()
                        .into_iter()
                        .map(|row| {
                            row.into_iter()
                                .map(|cell| cell.as_str().unwrap_or("").to_string())
                                .collect()
                        })
                        .collect()
                }
                Err(e) => {
                    debug!("Failed to get formulas for sheet {}: {}", sheet_meta.title, e);
                    Vec::new()
                }
            };
            
            sheets_data.push(SheetData {
                title: sheet_meta.title.clone(),
                id: sheet_meta.id,
                values,
                formulas,
            });
        }
        
        Ok(SpreadsheetData {
            title: metadata.title,
            sheets: sheets_data,
        })
    }

    pub async fn read_range(&self, spreadsheet_url_or_id: &str, range: &str) -> Result<Vec<Vec<String>>> {
        let spreadsheet_id = self.extract_spreadsheet_id(spreadsheet_url_or_id)?;
        
        info!("Reading range {} from spreadsheet: {}", range, spreadsheet_id);
        
        let result = self.sheets
            .spreadsheets()
            .values_get(&spreadsheet_id, range)
            .doit()
            .await
            .map_err(|e| anyhow!("Failed to read range {}: {}", range, e))?;
        
        let values = result.1.values.unwrap_or_default()
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|cell| cell.as_str().unwrap_or("").to_string())
                    .collect()
            })
            .collect();
        
        Ok(values)
    }

    pub fn to_markdown(&self, data: &SpreadsheetData) -> String {
        let mut lines = vec![format!("# {}\n", data.title)];
        
        for sheet in &data.sheets {
            lines.push(format!("\n## Sheet: {}\n", sheet.title));
            
            if sheet.values.is_empty() {
                lines.push("*Empty sheet*\n".to_string());
                continue;
            }
            
            // Find actual data range
            let non_empty_rows: Vec<usize> = sheet.values
                .iter()
                .enumerate()
                .filter_map(|(idx, row)| {
                    if row.iter().any(|cell| !cell.is_empty()) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            
            if non_empty_rows.is_empty() {
                lines.push("*Empty sheet*\n".to_string());
                continue;
            }
            
            let first_row = non_empty_rows[0];
            let last_row = non_empty_rows[non_empty_rows.len() - 1];
            
            // Find non-empty columns
            let mut non_empty_cols = std::collections::HashSet::new();
            for row_idx in first_row..=last_row {
                if let Some(row) = sheet.values.get(row_idx) {
                    for (col_idx, cell) in row.iter().enumerate() {
                        if !cell.is_empty() {
                            non_empty_cols.insert(col_idx);
                        }
                    }
                }
            }
            
            if non_empty_cols.is_empty() {
                lines.push("*Empty sheet*\n".to_string());
                continue;
            }
            
            let mut col_indices: Vec<usize> = non_empty_cols.into_iter().collect();
            col_indices.sort();
            
            // Create header with column letters
            let header_cols: Vec<String> = std::iter::once(" ".to_string())
                .chain(col_indices.iter().map(|&i| Self::col_number_to_letter(i + 1)))
                .collect();
            
            lines.push(format!("| {} |", header_cols.join(" | ")));
            lines.push(format!("|{}|", "---|".repeat(col_indices.len() + 1)));
            
            // Add data rows
            for row_idx in first_row..=last_row {
                let row_values = sheet.values.get(row_idx).cloned().unwrap_or_default();
                let row_formulas = sheet.formulas.get(row_idx).cloned().unwrap_or_default();
                
                let mut row_display = vec![(row_idx + 1).to_string()];
                
                for &col_idx in &col_indices {
                    let value = row_values.get(col_idx).cloned().unwrap_or_default();
                    let formula = row_formulas.get(col_idx).cloned().unwrap_or_default();
                    
                    let cell_display = if formula.starts_with('=') {
                        format!("`{}`", formula)
                    } else if value.is_empty() {
                        " ".to_string()
                    } else {
                        value
                    };
                    
                    row_display.push(cell_display);
                }
                
                lines.push(format!("| {} |", row_display.join(" | ")));
            }
        }
        
        lines.join("\n")
    }

    fn col_number_to_letter(col: usize) -> String {
        let mut result = String::new();
        let mut num = col;
        
        while num > 0 {
            num -= 1;
            let remainder = num % 26;
            result = format!("{}{}", (b'A' + remainder as u8) as char, result);
            num /= 26;
        }
        
        result
    }
}