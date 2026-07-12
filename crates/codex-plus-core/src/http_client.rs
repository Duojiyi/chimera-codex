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
