use dabgent_mcp::providers::{CombinedProvider, FilesystemProvider};
use rmcp::model::CallToolRequestParam;
use rmcp::ServiceExt;
use rmcp_in_process_transport::in_process::TokioInProcess;
use std::fs;
use std::path::PathBuf;

fn setup_work_dir(test_name: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir();
    let work_dir = temp_dir.join(format!("test_work_{}", test_name));

    // clean up if exists
    let _ = fs::remove_dir_all(&work_dir);

    work_dir
}

fn verify_work_dir_has_basic_files(work_dir: &PathBuf) {
    // just verify the directory exists and has some files
    assert!(work_dir.exists(), "work_dir should exist");

    // check for some expected files from template_minimal
    let has_files = work_dir.read_dir().unwrap().count() > 0;
    assert!(has_files, "work_dir should contain files from template");
}

#[tokio::test]
async fn test_normal_copy() {
    let work_dir = setup_work_dir("normal_copy");

    let filesystem = FilesystemProvider::new().unwrap();
    let provider = CombinedProvider::new(None, None, Some(filesystem)).unwrap();
    let tokio_in_process = TokioInProcess::new(provider).await.unwrap();
    let service = ().serve(tokio_in_process).await.unwrap();

    let args = serde_json::json!({
        "work_dir": work_dir.to_string_lossy(),
        "force_rewrite": false
    });

    let result = service
        .call_tool(CallToolRequestParam {
            name: "initiate_project".into(),
            arguments: Some(args.as_object().unwrap().clone()),
        })
        .await
        .unwrap();

    // verify success message
    let content = result.content.first().unwrap();
    let text = content.as_text().unwrap();
    assert!(text.text.contains("Successfully copied"));
    assert!(text.text.contains("from default template"));

    // verify files
    verify_work_dir_has_basic_files(&work_dir);

    // cleanup
    service.cancel().await.unwrap();
    fs::remove_dir_all(&work_dir).unwrap();
}

#[tokio::test]
async fn test_copy_twice_without_force() {
    let work_dir = setup_work_dir("copy_twice");

    let filesystem = FilesystemProvider::new().unwrap();
    let provider = CombinedProvider::new(None, None, Some(filesystem)).unwrap();
    let tokio_in_process = TokioInProcess::new(provider).await.unwrap();
    let service = ().serve(tokio_in_process).await.unwrap();

    let args = serde_json::json!({
        "work_dir": work_dir.to_string_lossy(),
        "force_rewrite": false
    });

    // first copy
    service
        .call_tool(CallToolRequestParam {
            name: "initiate_project".into(),
            arguments: Some(args.as_object().unwrap().clone()),
        })
        .await
        .unwrap();

    // second copy (should succeed, overwriting files)
    let result = service
        .call_tool(CallToolRequestParam {
            name: "initiate_project".into(),
            arguments: Some(args.as_object().unwrap().clone()),
        })
        .await
        .unwrap();

    let content = result.content.first().unwrap();
    let text = content.as_text().unwrap();
    assert!(text.text.contains("Successfully copied"));

    verify_work_dir_has_basic_files(&work_dir);

    // cleanup
    service.cancel().await.unwrap();
    fs::remove_dir_all(&work_dir).unwrap();
}

#[tokio::test]
async fn test_force_rewrite() {
    let work_dir = setup_work_dir("force_rewrite");

    let filesystem = FilesystemProvider::new().unwrap();
    let provider = CombinedProvider::new(None, None, Some(filesystem)).unwrap();
    let tokio_in_process = TokioInProcess::new(provider).await.unwrap();
    let service = ().serve(tokio_in_process).await.unwrap();

    let args = serde_json::json!({
        "work_dir": work_dir.to_string_lossy(),
        "force_rewrite": false
    });

    // first copy
    service
        .call_tool(CallToolRequestParam {
            name: "initiate_project".into(),
            arguments: Some(args.as_object().unwrap().clone()),
        })
        .await
        .unwrap();

    // add extra file
    fs::write(work_dir.join("extra_file.txt"), "should be deleted").unwrap();
    assert!(work_dir.join("extra_file.txt").exists());

    // force rewrite
    let args = serde_json::json!({
        "work_dir": work_dir.to_string_lossy(),
        "force_rewrite": true
    });

    let result = service
        .call_tool(CallToolRequestParam {
            name: "initiate_project".into(),
            arguments: Some(args.as_object().unwrap().clone()),
        })
        .await
        .unwrap();

    let content = result.content.first().unwrap();
    let text = content.as_text().unwrap();
    assert!(text.text.contains("Successfully copied"));

    // verify extra file was removed
    assert!(
        !work_dir.join("extra_file.txt").exists(),
        "extra_file.txt should be removed by force_rewrite"
    );

    verify_work_dir_has_basic_files(&work_dir);

    // cleanup
    service.cancel().await.unwrap();
    fs::remove_dir_all(&work_dir).unwrap();
}

#[tokio::test]
async fn test_with_default_template_path() {
    // verify the hardcoded default template path works
    let work_dir = setup_work_dir("default_template");

    let filesystem = FilesystemProvider::new().unwrap();
    let provider = CombinedProvider::new(None, None, Some(filesystem)).unwrap();
    let tokio_in_process = TokioInProcess::new(provider).await.unwrap();
    let service = ().serve(tokio_in_process).await.unwrap();

    let args = serde_json::json!({
        "work_dir": work_dir.to_string_lossy(),
        "force_rewrite": true
    });

    let result = service
        .call_tool(CallToolRequestParam {
            name: "initiate_project".into(),
            arguments: Some(args.as_object().unwrap().clone()),
        })
        .await
        .unwrap();

    let content = result.content.first().unwrap();
    let text = content.as_text().unwrap();
    assert!(text.text.contains("Successfully copied"));
    assert!(text.text.contains("from default template"));
    assert!(work_dir.exists());

    verify_work_dir_has_basic_files(&work_dir);

    // cleanup
    service.cancel().await.unwrap();
    fs::remove_dir_all(&work_dir).unwrap();
}
