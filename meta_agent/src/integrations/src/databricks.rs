use std::collections::HashMap;
use std::time::Duration;
use anyhow::{Result, anyhow};
use log::{info, debug};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::sleep;

const SQL_WAREHOUSES_ENDPOINT: &str = "/api/2.0/sql/warehouses";
const SQL_STATEMENTS_ENDPOINT: &str = "/api/2.0/sql/statements";
const UNITY_CATALOG_TABLES_ENDPOINT: &str = "/api/2.1/unity-catalog/tables";
const DEFAULT_WAIT_TIMEOUT: &str = "30s";
const MAX_POLL_ATTEMPTS: usize = 30;

#[derive(Debug, Deserialize)]
struct TableResponse {
    table_type: Option<String>,
    owner: Option<String>,
    comment: Option<String>,
    storage_location: Option<String>,
    data_source_format: Option<String>,
    columns: Option<Vec<TableColumn>>,
}

#[derive(Debug, Deserialize)]
struct TableColumn {
    name: Option<String>,
    type_name: Option<String>,
    comment: Option<String>,
}

#[derive(Debug)]
pub struct TableDetails {
    pub full_name: String,
    pub table_type: String,
    pub owner: Option<String>,
    pub comment: Option<String>,
    pub storage_location: Option<String>,
    pub data_source_format: Option<String>,
    pub columns: Vec<ColumnMetadata>,
    pub sample_data: Option<Vec<HashMap<String, Value>>>,
    pub row_count: Option<i64>,
}

#[derive(Debug)]
pub struct ColumnMetadata {
    pub name: String,
    pub data_type: String,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WarehouseListResponse {
    warehouses: Vec<Warehouse>,
}

#[derive(Debug, Deserialize)]
struct Warehouse {
    id: String,
    name: Option<String>,
    state: String,
}

#[derive(Debug, Serialize)]
struct SqlStatementRequest {
    statement: String,
    warehouse_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    row_limit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_limit: Option<i64>,
    disposition: String,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    wait_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_wait_timeout: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SqlStatementResponse {
    statement_id: String,
    status: Option<StatementStatus>,
    manifest: Option<ResultManifest>,
    result: Option<StatementResult>,
}

#[derive(Debug, Deserialize)]
struct StatementStatus {
    state: String,
    error: Option<StatementError>,
}

#[derive(Debug, Deserialize)]
struct StatementError {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResultManifest {
    schema: Option<Schema>,
}

#[derive(Debug, Deserialize)]
struct Schema {
    columns: Vec<Column>,
}

#[derive(Debug, Deserialize)]
struct Column {
    name: String,
}

#[derive(Debug, Deserialize)]
struct StatementResult {
    data_array: Option<Vec<Vec<Option<String>>>>,
}

pub struct DatabricksRestClient {
    host: String,
    token: String,
    client: reqwest::Client,
}

impl DatabricksRestClient {
    pub fn new() -> Result<Self> {
        let host = std::env::var("DATABRICKS_HOST")
            .map_err(|_| anyhow!("DATABRICKS_HOST environment variable not set"))?;
        let token = std::env::var("DATABRICKS_TOKEN")
            .map_err(|_| anyhow!("DATABRICKS_TOKEN environment variable not set"))?;

        let host = if host.starts_with("http") {
            host
        } else {
            format!("https://{}", host)
        };

        Ok(Self {
            host,
            token,
            client: reqwest::Client::new(),
        })
    }

    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization", 
            format!("Bearer {}", self.token).parse().unwrap()
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }

    async fn api_request<T>(&self, method: reqwest::Method, url: &str, body: Option<&impl Serialize>) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        debug!("{} {}", method, url);
        
        let mut request = self.client
            .request(method, url)
            .headers(self.auth_headers());
            
        if let Some(body) = body {
            request = request.json(body);
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("API request failed: {} - {}", status, error_text));
        }
        
        Ok(response.json().await?)
    }

    async fn list_warehouses(&self) -> Result<Vec<Warehouse>> {
        let url = format!("{}{}", self.host, SQL_WAREHOUSES_ENDPOINT);
        let response: WarehouseListResponse = self.api_request(reqwest::Method::GET, &url, None::<&()>).await?;
        debug!("Found {} warehouses", response.warehouses.len());
        Ok(response.warehouses)
    }

    pub fn format_value(value: &Value) -> String {
        match value {
            Value::String(s) => {
                if let Ok(num) = s.parse::<f64>() {
                    if s.contains('.') {
                        format!("{:.2}", num)
                    } else {
                        num.to_string()
                    }
                } else {
                    s.clone()
                }
            }
            Value::Null => "N/A".to_string(),
            _ => format!("{}", value),
        }
    }

    async fn find_warehouse_id(&self) -> Result<String> {
        if let Ok(warehouse_id) = std::env::var("DATABRICKS_WAREHOUSE_ID") {
            return Ok(warehouse_id);
        }

        let warehouses = self.list_warehouses().await?;
        
        let running_warehouses: Vec<&Warehouse> = warehouses
            .iter()
            .filter(|w| w.state == "RUNNING")
            .collect();
        
        if !running_warehouses.is_empty() {
            let warehouse = running_warehouses[0];
            info!("Found running warehouse: {} ({})", 
                  warehouse.name.as_ref().unwrap_or(&warehouse.id), warehouse.id);
            return Ok(warehouse.id.clone());
        }
        
        if !warehouses.is_empty() {
            let warehouse = &warehouses[0];
            info!("No running warehouse found, using first available: {} ({})", 
                  warehouse.name.as_ref().unwrap_or(&warehouse.id), warehouse.id);
            return Ok(warehouse.id.clone());
        }
        
        Err(anyhow!("No warehouses found in this Databricks workspace"))
    }

    pub async fn execute_sql(&self, query: &str) -> Result<Vec<HashMap<String, Value>>> {
        let warehouse_id = self.find_warehouse_id().await?;
        
        info!("Executing query on warehouse: {}", warehouse_id);
        debug!("Query: {}", query.replace('\n', " "));
        
        let request = SqlStatementRequest {
            statement: query.to_string(),
            warehouse_id,
            catalog: None,
            schema: None,
            parameters: None,
            row_limit: None,
            byte_limit: None,
            disposition: "INLINE".to_string(),
            format: "JSON_ARRAY".to_string(),
            wait_timeout: Some(DEFAULT_WAIT_TIMEOUT.to_string()),
            on_wait_timeout: Some("CONTINUE".to_string()),
        };

        let url = format!("{}{}", self.host, SQL_STATEMENTS_ENDPOINT);
        let mut sql_response: SqlStatementResponse = self.api_request(reqwest::Method::POST, &url, Some(&request)).await?;
        
        if let Some(status) = &sql_response.status {
            if status.state == "PENDING" || status.state == "RUNNING" {
                sql_response = self.poll_statement(&sql_response.statement_id).await?;
            }
        }

        self.validate_sql_response(&sql_response)?;

        self.convert_response_to_hash_map(&sql_response)
    }

    fn validate_sql_response(&self, response: &SqlStatementResponse) -> Result<()> {
        match &response.status {
            Some(status) => {
                if status.state == "SUCCEEDED" {
                    debug!("Query executed successfully");
                    Ok(())
                } else {
                    let error_msg = status.error
                        .as_ref()
                        .and_then(|e| e.message.as_deref())
                        .map(|msg| format!("Query failed with state: {} - {}", status.state, msg))
                        .unwrap_or_else(|| format!("Query failed with state: {}", status.state));
                    Err(anyhow!(error_msg))
                }
            }
            None => Err(anyhow!("Query execution status is None")),
        }
    }

    pub async fn get_table_details(&self, table_full_name: &str, sample_size: usize) -> Result<TableDetails> {
        info!("Getting details for table: {}", table_full_name);

        let parts: Vec<&str> = table_full_name.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!(
                "Invalid table name format: {}. Expected catalog.schema.table", 
                table_full_name
            ));
        }

        let table_metadata = self.get_table_metadata(table_full_name).await?;
        
        let sample_data = self.get_sample_data(table_full_name, sample_size).await?;
        
        let row_count = self.get_row_count(table_full_name).await?;

        Ok(TableDetails {
            full_name: table_full_name.to_string(),
            table_type: table_metadata.table_type.unwrap_or_else(|| "UNKNOWN".to_string()),
            owner: table_metadata.owner,
            comment: table_metadata.comment,
            storage_location: table_metadata.storage_location,
            data_source_format: table_metadata.data_source_format,
            columns: table_metadata.columns.unwrap_or_default()
                .into_iter()
                .filter_map(|col| {
                    if let (Some(name), Some(type_name)) = (col.name, col.type_name) {
                        Some(ColumnMetadata {
                            name,
                            data_type: type_name,
                            comment: col.comment,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            sample_data: Some(sample_data),
            row_count: Some(row_count),
        })
    }

    async fn get_table_metadata(&self, table_full_name: &str) -> Result<TableResponse> {
        let url = format!("{}{}/{}", self.host, UNITY_CATALOG_TABLES_ENDPOINT, table_full_name);
        self.api_request(reqwest::Method::GET, &url, None::<&()>).await
    }

    async fn get_sample_data(&self, table_full_name: &str, sample_size: usize) -> Result<Vec<HashMap<String, Value>>> {
        let sample_query = format!("SELECT * FROM {} LIMIT {}", table_full_name, sample_size);
        self.execute_sql(&sample_query).await
    }

    async fn get_row_count(&self, table_full_name: &str) -> Result<i64> {
        let count_query = format!("SELECT COUNT(*) as count FROM {}", table_full_name);
        let results = self.execute_sql(&count_query).await?;
        
        results.first()
            .and_then(|row| row.get("count"))
            .and_then(|v| match v {
                Value::String(s) => s.parse().ok(),
                _ => None,
            })
            .ok_or_else(|| anyhow!("Count query returned invalid result"))
    }

    async fn poll_statement(&self, statement_id: &str) -> Result<SqlStatementResponse> {
        let url = format!("{}{}/{}", self.host, SQL_STATEMENTS_ENDPOINT, statement_id);
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > MAX_POLL_ATTEMPTS {
                return Err(anyhow!("Statement execution timed out after {} attempts", MAX_POLL_ATTEMPTS));
            }

            let sql_response: SqlStatementResponse = self.api_request(reqwest::Method::GET, &url, None::<&()>).await?;
            
            if let Some(status) = &sql_response.status {
                match status.state.as_str() {
                    "SUCCEEDED" | "FAILED" | "CANCELED" => return Ok(sql_response),
                    "PENDING" | "RUNNING" => {
                        debug!("Statement still running, attempt {}/{}", attempts, MAX_POLL_ATTEMPTS);
                        sleep(Duration::from_millis(1000)).await;
                        continue;
                    }
                    _ => {
                        debug!("Unknown state: {}, continuing to poll", status.state);
                        sleep(Duration::from_millis(1000)).await;
                        continue;
                    }
                }
            } else {
                return Err(anyhow!("No status in response"));
            }
        }
    }

    fn convert_response_to_hash_map(&self, response: &SqlStatementResponse) -> Result<Vec<HashMap<String, Value>>> {
        match (&response.result, &response.manifest) {
            (Some(result), Some(manifest)) => {
                match (&result.data_array, &manifest.schema) {
                    (Some(data_array), Some(schema)) => {
                        let col_names: Vec<String> = schema.columns
                            .iter()
                            .map(|col| col.name.clone())
                            .collect();
                        
                        let rows: Vec<HashMap<String, Value>> = data_array
                            .iter()
                            .map(|row| {
                                col_names
                                    .iter()
                                    .zip(row.iter())
                                    .map(|(name, value)| {
                                        let json_value = match value {
                                            Some(s) => Value::String(s.clone()),
                                            None => Value::Null,
                                        };
                                        (name.clone(), json_value)
                                    })
                                    .collect()
                            })
                            .collect();
                        
                        Ok(rows)
                    }
                    _ => Err(anyhow!("Result data_array or manifest schema is None")),
                }
            }
            _ => Ok(Vec::new()),
        }
    }
}