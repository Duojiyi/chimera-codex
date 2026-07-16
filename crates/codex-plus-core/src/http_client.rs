pub fn branded_user_agent(component: &str) -> String {
    let component = component.trim();
    let component = if component.is_empty() {
        env!("CARGO_PKG_VERSION")
    } else {
        component
    };
    format!("{}/{component}", crate::branding::ARTIFACT_PREFIX)
}

pub fn proxied_client(user_agent: &str) -> anyhow::Result<reqwest::Client> {
    let ua = if user_agent.trim().is_empty() {
        branded_user_agent("")
    } else {
        user_agent.trim().to_string()
    };
    Ok(reqwest::Client::builder().user_agent(ua).build()?)
}

/// VLM 专用 HTTP client（带超时）。
/// 不复用通用 proxied_client，避免 VLM 服务无响应时永久阻塞整个代理。
pub fn vlm_http_client() -> anyhow::Result<reqwest::Client> {
    vlm_http_client_with_timeout(
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(30),
    )
}

pub(crate) fn vlm_http_client_with_timeout(
    connect: std::time::Duration,
    total: std::time::Duration,
) -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(format!("CodexPlusPlus-VLM/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(connect)
        .timeout(total)
        .build()?)
}
