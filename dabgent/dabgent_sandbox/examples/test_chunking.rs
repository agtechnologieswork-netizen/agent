use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};
use dabgent_sandbox::Sandbox;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🧪 Testing chunking approach for write_files...");

    let opts = ConnectOpts::default();
    opts.connect(|client| async move {
        // Create a simple container
        let ctr = client.container().from("alpine:latest");
        let mut sandbox = DaggerSandbox::from_container(ctr, client.clone());

        // Generate thousands of files to stress test chunking approach
        let mut test_files = Vec::new();
        for i in 1..=2000 {
            let path = format!("/test/batch_{:03}/file_{:04}.txt", i / 100, i);
            let content = format!("This is test file number {}\nCreated to stress test chunking approach\nBatch: {}\nTimestamp: {}", i, i / 100, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
            test_files.push((path, content));
        }

        println!("📁 Creating {} test files to verify chunking works...", test_files.len());

        // Convert to references for the API
        let files_refs: Vec<(&str, &str)> = test_files.iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();

        // This should work with chunking, but would fail with the old approach
        sandbox.write_files(files_refs).await?;

        println!("✅ Successfully wrote {} files using chunking approach!", test_files.len());

        // Verify a few files were actually written from different batches
        let test_content_1 = sandbox.read_file("/test/batch_000/file_0001.txt").await?;
        let test_content_1000 = sandbox.read_file("/test/batch_010/file_1000.txt").await?;
        let test_content_2000 = sandbox.read_file("/test/batch_020/file_2000.txt").await?;

        println!("📄 Verified file contents:");
        println!("  file_0001.txt: {}", test_content_1.lines().next().unwrap_or(""));
        println!("  file_1000.txt: {}", test_content_1000.lines().next().unwrap_or(""));
        println!("  file_2000.txt: {}", test_content_2000.lines().next().unwrap_or(""));

        // List the directory to see all batches
        let entries = sandbox.list_directory("/test").await?;
        println!("📋 Directory listing (/test): {} batch directories", entries.len());

        println!("🎉 Chunking approach test completed successfully!");
        Ok(())
    }).await?;

    Ok(())
}