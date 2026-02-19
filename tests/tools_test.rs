use rustclaw::tools::Tool;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn shell_echo_captures_output() {
    let tmp = TempDir::new().unwrap();
    let tool = rustclaw::tools::shell::ShellTool::new(tmp.path().to_path_buf());
    let out = tool.run(json!({"command": "echo hello world"})).await.unwrap();
    assert_eq!(out["exit_code"], 0);
    assert!(out["stdout"].as_str().unwrap().contains("hello world"));
}

#[tokio::test]
async fn shell_rejects_dangerous_commands() {
    let tmp = TempDir::new().unwrap();
    let tool = rustclaw::tools::shell::ShellTool::new(tmp.path().to_path_buf());

    // rm should be blocked
    let result = tool.run(json!({"command": "rm -rf /tmp/test"})).await;
    assert!(result.is_err());

    // sudo should be blocked
    let result = tool.run(json!({"command": "sudo apt install malware"})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn shell_captures_env_vars() {
    let tmp = TempDir::new().unwrap();
    let tool = rustclaw::tools::shell::ShellTool::new(tmp.path().to_path_buf());
    let out = tool.run(json!({"command": "echo $TEST_VAR", "env": {"TEST_VAR": "hello123"}})).await.unwrap();
    assert!(out["stdout"].as_str().unwrap().contains("hello123"));
}

#[tokio::test]
async fn shell_timeout_enforced() {
    let tmp = TempDir::new().unwrap();
    let tool = rustclaw::tools::shell::ShellTool::new(tmp.path().to_path_buf());
    let result = tool.run(json!({"command": "sleep 60", "timeout_secs": 1})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timed out"));
}

#[tokio::test]
async fn file_tool_path_traversal_blocked() {
    let tmp = TempDir::new().unwrap();
    let tool = rustclaw::tools::file::FileTool::new(tmp.path().to_path_buf());

    // Absolute path outside workspace
    let result = tool.run(json!({"action": "read", "path": "/etc/passwd"})).await;
    assert!(result.is_err());

    // Relative path escaping workspace
    let result = tool.run(json!({"action": "read", "path": "../../../etc/passwd"})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn file_tool_creates_parent_dirs() {
    let tmp = TempDir::new().unwrap();
    let tool = rustclaw::tools::file::FileTool::new(tmp.path().to_path_buf());
    tool.run(json!({"action": "write", "path": "deep/nested/dir/file.txt", "content": "test"})).await.unwrap();
    let out = tool.run(json!({"action": "read", "path": "deep/nested/dir/file.txt"})).await.unwrap();
    assert_eq!(out["content"], "test");
}

#[tokio::test]
async fn process_tool_list_and_kill() {
    let tool = rustclaw::tools::process::ProcessTool::new();
    let list = tool.run(json!({"action": "list"})).await.unwrap();
    assert_eq!(list["sessions"].as_array().unwrap().len(), 0);

    // Kill nonexistent session
    let result = tool.run(json!({"action": "kill", "session_id": "nonexistent"})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn browser_tool_http_fallback() {
    let tool = rustclaw::tools::browser::BrowserTool;
    let out = tool.run(json!({"action": "open", "url": "https://example.com"})).await.unwrap();
    assert!(out["bytes"].as_u64().unwrap() > 0);
    assert_eq!(out["method"], "http_fetch");
}

#[tokio::test]
async fn browser_snapshot_requires_cdp() {
    let tool = rustclaw::tools::browser::BrowserTool;
    let result = tool.run(json!({"action": "snapshot"})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn web_search_tool_falls_back_to_ddg() {
    let tool = rustclaw::tools::web::WebSearchTool { allowed_domains: vec![] };
    // Without BRAVE_API_KEY set, should fall back to DuckDuckGo (not error)
    if std::env::var("BRAVE_API_KEY").is_err() {
        let result = tool.run(json!({"query": "test"})).await;
        assert!(result.is_ok(), "should fall back to DuckDuckGo when BRAVE_API_KEY not set");
    }
}
