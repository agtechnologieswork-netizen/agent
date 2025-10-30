use anyhow::Result;
use edda_integrations::{
    DatabricksRestClient, DescribeTableRequest, ExecuteSqlRequest, ToolResultDisplay,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("=== Databricks REST API Integration Example ===");

    let client = DatabricksRestClient::new()?;

    // Example: Simple test query first
    println!("\n=== Simple Test Query ===");
    let simple_query_request = ExecuteSqlRequest {
        query: "SELECT 1 as test_value".to_string(),
    };

    match client.execute_sql(&simple_query_request).await {
        Ok(result) => {
            println!("Simple query successful!");
            println!("{}", result.display());
        }
        Err(e) => println!("Simple query failed: {}", e),
    }

    // Example: Core sales metrics
    println!("\n=== Core Sales Metrics ===");
    let metrics_request = ExecuteSqlRequest {
        query: r#"
            SELECT
                CAST(SUM(totalPrice) AS DECIMAL(10,2)) as total_revenue,
                COUNT(*) as total_orders,
                COUNT(DISTINCT customerID) as unique_customers,
                CAST(AVG(totalPrice) AS DECIMAL(10,2)) as average_order_value
            FROM samples.bakehouse.sales_transactions
        "#
        .to_string(),
    };

    match client.execute_sql(&metrics_request).await {
        Ok(result) => {
            println!("{}", result.display());
        }
        Err(e) => println!("Error fetching sales metrics: {}", e),
    }

    // Example: Table details
    println!("\n=== Table Details Example ===");
    let table_request = DescribeTableRequest {
        table_full_name: "samples.bakehouse.sales_transactions".to_string(),
        sample_size: 3,
    };

    match client.describe_table(&table_request).await {
        Ok(details) => {
            println!("{}", details.display());
        }
        Err(e) => println!("Error fetching table details: {}", e),
    }

    println!("\n=== Integration example completed ===");
    Ok(())
}
