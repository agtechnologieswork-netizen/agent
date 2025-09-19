use anyhow::Result;
use dabgent_integrations::DatabricksRestClient;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("=== Databricks REST API Integration Example ===");

    let client = DatabricksRestClient::new()?;

    // Example: Simple test query first
    println!("\n=== Simple Test Query ===");
    let simple_query = "SELECT 1 as test_value";

    match client.execute_sql(simple_query).await {
        Ok(results) => {
            println!("Simple query successful! Results:");
            for row in results {
                println!("  {:?}", row);
            }
        }
        Err(e) => println!("Simple query failed: {}", e),
    }

    // Example: Core sales metrics
    println!("\n=== Core Sales Metrics ===");
    let metrics_query = r#"
        SELECT 
            CAST(SUM(totalPrice) AS DECIMAL(10,2)) as total_revenue,
            COUNT(*) as total_orders,
            COUNT(DISTINCT customerID) as unique_customers,
            CAST(AVG(totalPrice) AS DECIMAL(10,2)) as average_order_value
        FROM samples.bakehouse.sales_transactions
    "#;

    match client.execute_sql(metrics_query).await {
        Ok(results) => {
            for row in results {
                println!(
                    "Total Revenue: ${}",
                    DatabricksRestClient::format_value(
                        row.get("total_revenue").unwrap_or(&serde_json::Value::Null)
                    )
                );
                println!(
                    "Total Orders: {}",
                    DatabricksRestClient::format_value(
                        row.get("total_orders").unwrap_or(&serde_json::Value::Null)
                    )
                );
                println!(
                    "Unique Customers: {}",
                    DatabricksRestClient::format_value(
                        row.get("unique_customers")
                            .unwrap_or(&serde_json::Value::Null)
                    )
                );
                println!(
                    "Average Order Value: ${}",
                    DatabricksRestClient::format_value(
                        row.get("average_order_value")
                            .unwrap_or(&serde_json::Value::Null)
                    )
                );
            }
        }
        Err(e) => println!("Error fetching sales metrics: {}", e),
    }

    // Example: Table details
    println!("\n=== Table Details Example ===");
    let table_name = "samples.bakehouse.sales_transactions";
    match client.get_table_details(table_name, 3).await {
        Ok(details) => {
            println!("Table: {}", details.full_name);
            println!("Type: {}", details.table_type);
            println!("Owner: {}", details.owner.as_deref().unwrap_or("N/A"));
            println!(
                "Row Count: {}",
                details
                    .row_count
                    .map(|c| c.to_string())
                    .as_deref()
                    .unwrap_or("N/A")
            );

            println!("\nColumns ({}):", details.columns.len());
            for col in &details.columns {
                println!("  - {} ({})", col.name, col.data_type);
            }

            if let Some(sample_data) = &details.sample_data {
                println!("\nSample Data ({} rows):", sample_data.len());
                for (i, row) in sample_data.iter().take(2).enumerate() {
                    println!("  Row {}: {:?}", i + 1, row);
                }
            }
        }
        Err(e) => println!("Error fetching table details: {}", e),
    }

    println!("\n=== Integration example completed ===");
    Ok(())
}
