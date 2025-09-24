use anyhow::{Result, anyhow};
use log::{debug, info};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
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
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }

    async fn api_request<T>(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        debug!("Making {} request to {}", method, url);

        let mut request = self
            .client
            .request(method, url)
            .headers(self.auth_headers());

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response text: {}", e))?;

        debug!("Response status: {}, body: {}", status, response_text);

        if !status.is_success() {
            return Err(anyhow!(
                "API request failed with status {}: {}",
                status,
                response_text
            ));
        }

        serde_json::from_str(&response_text).map_err(|e| {
            anyhow!(
                "Failed to parse JSON response: {}. Response: {}",
                e,
                response_text
            )
        })
    }

    async fn get_available_warehouse(&self) -> Result<String> {
        let url = format!("{}{}", self.host, SQL_WAREHOUSES_ENDPOINT);
        let response: WarehouseListResponse = self
            .api_request(reqwest::Method::GET, &url, None::<&()>)
            .await?;

        let running_warehouse = response
            .warehouses
            .into_iter()
            .find(|w| w.state == "RUNNING")
            .ok_or_else(|| anyhow!("No running SQL warehouse found"))?;

        info!(
            "Using warehouse: {} (ID: {})",
            running_warehouse.name.as_deref().unwrap_or("Unknown"),
            running_warehouse.id
        );

        Ok(running_warehouse.id)
    }

    pub async fn execute_sql(&self, sql: &str) -> Result<Vec<HashMap<String, Value>>> {
        let warehouse_id = self.get_available_warehouse().await?;

        let request = SqlStatementRequest {
            statement: sql.to_string(),
            warehouse_id,
            catalog: None,
            schema: None,
            parameters: None,
            row_limit: Some(100),
            byte_limit: None,
            disposition: "INLINE".to_string(),
            format: "JSON_ARRAY".to_string(),
            wait_timeout: Some(DEFAULT_WAIT_TIMEOUT.to_string()),
            on_wait_timeout: Some("CONTINUE".to_string()),
        };

        let url = format!("{}{}", self.host, SQL_STATEMENTS_ENDPOINT);
        let response: SqlStatementResponse = self
            .api_request(reqwest::Method::POST, &url, Some(&request))
            .await?;

        // Check if we need to poll for results
        if let Some(status) = &response.status {
            if status.state == "PENDING" || status.state == "RUNNING" {
                return self.poll_for_results(&response.statement_id).await;
            } else if status.state == "FAILED" {
                let error_msg = status
                    .error
                    .as_ref()
                    .and_then(|e| e.message.as_ref())
                    .map(|m| m.as_str())
                    .unwrap_or("Unknown error");
                return Err(anyhow!("SQL execution failed: {}", error_msg));
            }
        }

        self.process_statement_result(&response)
    }

    async fn poll_for_results(&self, statement_id: &str) -> Result<Vec<HashMap<String, Value>>> {
        for attempt in 0..MAX_POLL_ATTEMPTS {
            debug!(
                "Polling attempt {} for statement {}",
                attempt + 1,
                statement_id
            );

            let url = format!("{}{}/{}", self.host, SQL_STATEMENTS_ENDPOINT, statement_id);
            let response: SqlStatementResponse = self
                .api_request(reqwest::Method::GET, &url, None::<&()>)
                .await?;

            if let Some(status) = &response.status {
                match status.state.as_str() {
                    "SUCCEEDED" => return self.process_statement_result(&response),
                    "FAILED" => {
                        let error_msg = status
                            .error
                            .as_ref()
                            .and_then(|e| e.message.as_ref())
                            .map(|m| m.as_str())
                            .unwrap_or("Unknown error");
                        return Err(anyhow!("SQL execution failed: {}", error_msg));
                    }
                    "PENDING" | "RUNNING" => {
                        sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    _ => return Err(anyhow!("Unexpected statement state: {}", status.state)),
                }
            }
        }

        Err(anyhow!(
            "Polling timeout exceeded for statement {}",
            statement_id
        ))
    }

    fn process_statement_result(
        &self,
        response: &SqlStatementResponse,
    ) -> Result<Vec<HashMap<String, Value>>> {
        debug!("Processing statement result: {:?}", response);

        let schema = response
            .manifest
            .as_ref()
            .and_then(|m| m.schema.as_ref())
            .ok_or_else(|| anyhow!("No schema in response"))?;

        // Try to get inline data
        if let Some(result) = &response.result {
            if let Some(data_array) = &result.data_array {
                debug!("Found {} rows of inline data", data_array.len());
                return self.process_data_array(schema, data_array);
            }
        }

        debug!(
            "Response structure: manifest={:?}, result={:?}",
            response.manifest, response.result
        );
        Err(anyhow!("No data found in response"))
    }

    fn process_data_array(
        &self,
        schema: &Schema,
        data_array: &[Vec<Option<String>>],
    ) -> Result<Vec<HashMap<String, Value>>> {
        let mut results = Vec::new();

        for row in data_array {
            let mut row_map = HashMap::new();

            for (i, column) in schema.columns.iter().enumerate() {
                let value = row
                    .get(i)
                    .and_then(|v| v.as_ref())
                    .map(|s| {
                        // Try to parse as number first, then as string
                        if let Ok(num) = s.parse::<f64>() {
                            Value::Number(
                                serde_json::Number::from_f64(num)
                                    .unwrap_or_else(|| serde_json::Number::from(0)),
                            )
                        } else {
                            Value::String(s.clone())
                        }
                    })
                    .unwrap_or(Value::Null);

                row_map.insert(column.name.clone(), value);
            }

            results.push(row_map);
        }

        Ok(results)
    }

    pub async fn get_table_details(
        &self,
        table_name: &str,
        sample_rows: usize,
    ) -> Result<TableDetails> {
        // Get basic table metadata from Unity Catalog
        let url = format!(
            "{}{}/{}",
            self.host, UNITY_CATALOG_TABLES_ENDPOINT, table_name
        );
        let table_response: TableResponse = self
            .api_request(reqwest::Method::GET, &url, None::<&()>)
            .await?;

        // Build column metadata
        let columns = table_response
            .columns
            .unwrap_or_default()
            .into_iter()
            .map(|col| ColumnMetadata {
                name: col.name.unwrap_or_else(|| "unknown".to_string()),
                data_type: col.type_name.unwrap_or_else(|| "unknown".to_string()),
                comment: col.comment,
            })
            .collect();

        // Get sample data and row count
        let sample_data = if sample_rows > 0 {
            let sql = format!("SELECT * FROM {} LIMIT {}", table_name, sample_rows);
            self.execute_sql(&sql).await.ok()
        } else {
            None
        };

        let row_count = {
            let sql = format!("SELECT COUNT(*) as count FROM {}", table_name);
            self.execute_sql(&sql)
                .await
                .ok()
                .and_then(|results| results.first().cloned())
                .and_then(|row| row.get("count").cloned())
                .and_then(|value| match value {
                    Value::Number(n) => n.as_i64(),
                    Value::String(s) => s.parse().ok(),
                    _ => None,
                })
        };

        Ok(TableDetails {
            full_name: table_name.to_string(),
            table_type: table_response
                .table_type
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            owner: table_response.owner,
            comment: table_response.comment,
            storage_location: table_response.storage_location,
            data_source_format: table_response.data_source_format,
            columns,
            sample_data,
            row_count,
        })
    }

    pub fn format_value(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            _ => format!("{:?}", value),
        }
    }
}
