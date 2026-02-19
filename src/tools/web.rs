use crate::config::WebConfig;
use crate::tools::Tool;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::{Value, json};

/// Web search tool with URL allowlist and DuckDuckGo fallback.
pub struct WebSearchTool {
    pub allowed_domains: Vec<String>,
}

/// Web fetch tool with URL allowlist.
pub struct WebFetchTool {
    pub allowed_domains: Vec<String>,
}

impl WebSearchTool {
    pub fn new(web_cfg: &WebConfig) -> Self {
        Self {
            allowed_domains: web_cfg.allowed_domains.clone(),
        }
    }
}

impl WebFetchTool {
    pub fn new(web_cfg: &WebConfig) -> Self {
        Self {
            allowed_domains: web_cfg.allowed_domains.clone(),
        }
    }
}

#[derive(Deserialize)]
struct SearchReq { query: String, count: Option<u32> }

#[derive(Deserialize)]
struct FetchReq { url: String, mode: Option<String> }

/// Check if a URL's domain is in the allowed list.
/// Returns Ok(()) if allowed, Err with message if blocked.
pub fn check_domain_allowed(url_str: &str, allowed_domains: &[String]) -> Result<()> {
    if allowed_domains.is_empty() {
        return Ok(());
    }
    let parsed = url::Url::parse(url_str).map_err(|e| anyhow!("invalid URL: {e}"))?;
    let host = parsed.host_str().ok_or_else(|| anyhow!("URL has no host"))?;
    for allowed in allowed_domains {
        if host == allowed || host.ends_with(&format!(".{allowed}")) {
            return Ok(());
        }
    }
    Err(anyhow!("domain '{host}' is not in the allowed domains list"))
}

/// Search via Brave API.
async fn brave_search(query: &str, count: u32, api_key: &str) -> Result<Value> {
    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
        urlencoding::encode(query),
        count
    );
    let val: Value = reqwest::Client::new()
        .get(url)
        .header("X-Subscription-Token", api_key)
        .send()
        .await?
        .json()
        .await?;
    Ok(val)
}

/// Search via DuckDuckGo HTML scraping (fallback when BRAVE_API_KEY is absent).
async fn ddg_search(query: &str, count: u32) -> Result<Value> {
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );
    let html = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (compatible; rustclaw/0.1)")
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&html);
    let result_sel = Selector::parse(".result__a").unwrap();
    let snippet_sel = Selector::parse(".result__snippet").unwrap();

    let titles: Vec<_> = doc.select(&result_sel).collect();
    let snippets: Vec<_> = doc.select(&snippet_sel).collect();

    let mut results = Vec::new();
    for i in 0..(count as usize).min(titles.len()) {
        let title = titles[i].text().collect::<Vec<_>>().join("");
        let href = titles[i].value().attr("href").unwrap_or("");
        let actual_url = decode_ddg_redirect(href);
        let snippet = snippets
            .get(i)
            .map(|s| s.text().collect::<Vec<_>>().join(""))
            .unwrap_or_default();
        results.push(json!({
            "title": title.trim(),
            "url": actual_url,
            "description": snippet.trim(),
        }));
    }

    Ok(json!({
        "web": { "results": results },
        "provider": "duckduckgo"
    }))
}

/// Decode DuckDuckGo redirect URLs.
fn decode_ddg_redirect(href: &str) -> String {
    if let Some(pos) = href.find("uddg=") {
        let encoded = &href[pos + 5..];
        let end = encoded.find('&').unwrap_or(encoded.len());
        urlencoding::decode(&encoded[..end])
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| href.to_string())
    } else {
        href.to_string()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str { "web_search" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: SearchReq = serde_json::from_value(input)?;
        let count = req.count.unwrap_or(5);

        // Try Brave first if API key is set, otherwise fallback to DDG
        match std::env::var("BRAVE_API_KEY") {
            Ok(key) if !key.is_empty() => brave_search(&req.query, count, &key).await,
            _ => {
                tracing::info!("BRAVE_API_KEY not set, using DuckDuckGo fallback");
                ddg_search(&req.query, count).await
            }
        }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str { "web_fetch" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: FetchReq = serde_json::from_value(input)?;

        // Check domain allowlist
        check_domain_allowed(&req.url, &self.allowed_domains)?;

        let html = reqwest::get(&req.url).await?.text().await?;
        if req.mode.as_deref() == Some("text") {
            let doc = Html::parse_document(&html);
            let sel = Selector::parse("body").unwrap();
            let text = doc
                .select(&sel)
                .next()
                .map(|n| n.text().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            return Ok(json!({"content": text}));
        }
        Ok(json!({"content": html}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- Feature 1: URL allowlist tests --

    #[test]
    fn domain_allowed_when_list_empty() {
        assert!(check_domain_allowed("https://example.com/path", &[]).is_ok());
    }

    #[test]
    fn domain_allowed_exact_match() {
        let allowed = vec!["example.com".into()];
        assert!(check_domain_allowed("https://example.com/path", &allowed).is_ok());
    }

    #[test]
    fn domain_allowed_subdomain_match() {
        let allowed = vec!["example.com".into()];
        assert!(check_domain_allowed("https://sub.example.com/path", &allowed).is_ok());
    }

    #[test]
    fn domain_rejected_when_not_in_list() {
        let allowed = vec!["example.com".into()];
        let result = check_domain_allowed("https://evil.com/path", &allowed);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not in the allowed domains list"));
    }

    #[test]
    fn domain_rejected_partial_suffix_no_match() {
        let allowed = vec!["example.com".into()];
        let result = check_domain_allowed("https://notexample.com/path", &allowed);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_url_rejected() {
        let allowed = vec!["example.com".into()];
        let result = check_domain_allowed("not-a-url", &allowed);
        assert!(result.is_err());
    }

    // -- Feature 2: DuckDuckGo fallback tests --

    #[test]
    fn ddg_redirect_decode() {
        let href = "/l/?uddg=https%3A%2F%2Fexample.com%2Fpage&rut=abc";
        let decoded = decode_ddg_redirect(href);
        assert_eq!(decoded, "https://example.com/page");
    }

    #[test]
    fn ddg_redirect_no_uddg() {
        let href = "https://direct.com/page";
        let decoded = decode_ddg_redirect(href);
        assert_eq!(decoded, "https://direct.com/page");
    }

    #[tokio::test]
    async fn web_fetch_text_works() {
        let tool = WebFetchTool::new(&crate::config::WebConfig::default());
        let out = tool
            .run(json!({"url":"https://example.com","mode":"text"}))
            .await
            .unwrap();
        assert!(out["content"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("example"));
    }

    #[tokio::test]
    async fn web_fetch_blocked_by_allowlist() {
        let cfg = crate::config::WebConfig {
            allowed_domains: vec!["allowed.com".into()],
        };
        let tool = WebFetchTool::new(&cfg);
        let result = tool.run(json!({"url":"https://blocked.com/page"})).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not in the allowed domains list"));
    }

    #[tokio::test]
    async fn web_fetch_allowed_by_allowlist() {
        let cfg = crate::config::WebConfig {
            allowed_domains: vec!["example.com".into()],
        };
        let tool = WebFetchTool::new(&cfg);
        let out = tool
            .run(json!({"url":"https://example.com","mode":"text"}))
            .await
            .unwrap();
        assert!(out["content"].as_str().is_some());
    }
}
