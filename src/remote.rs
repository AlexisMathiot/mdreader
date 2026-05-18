use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const USER_AGENT: &str = concat!("mdreader/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(10);

pub enum Input {
    Local(PathBuf),
    Url(String),
    Github { owner: String, repo: String },
}

pub struct Fetched {
    pub content: String,
    pub display: String,
    pub raw_url: String,
}

pub fn parse(arg: &str) -> Input {
    if arg.starts_with("http://") || arg.starts_with("https://") {
        return Input::Url(arg.to_string());
    }
    if let Some((owner, repo)) = parse_github_repo(arg)
        && !PathBuf::from(arg).exists()
    {
        return Input::Github { owner, repo };
    }
    Input::Local(PathBuf::from(arg))
}

fn parse_github_repo(s: &str) -> Option<(String, String)> {
    let (owner, repo) = s.split_once('/')?;
    let valid =
        |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if valid(owner) && valid(repo) {
        Some((owner.into(), repo.into()))
    } else {
        None
    }
}

pub fn fetch(input: &Input) -> Result<Fetched> {
    match input {
        Input::Url(url) => fetch_url(url, url),
        Input::Github { owner, repo } => fetch_github_readme(owner, repo),
        Input::Local(_) => bail!("local inputs don't go through fetch()"),
    }
}

pub fn refetch(raw_url: &str) -> Result<String> {
    let resp = client()?
        .get(raw_url)
        .send()
        .with_context(|| format!("GET {raw_url}"))?
        .error_for_status()?;
    Ok(resp.text()?)
}

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT)
        .build()
        .context("build HTTP client")
}

fn fetch_url(url: &str, display_source: &str) -> Result<Fetched> {
    let resp = client()?
        .get(url)
        .send()
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?;
    let final_url = resp.url().to_string();
    let content = resp.text()?;
    Ok(Fetched {
        content,
        display: short_url(display_source),
        raw_url: final_url,
    })
}

#[derive(Deserialize)]
struct GithubReadme {
    download_url: String,
}

fn fetch_github_readme(owner: &str, repo: &str) -> Result<Fetched> {
    let api = format!("https://api.github.com/repos/{owner}/{repo}/readme");
    let mut req = client()?
        .get(&api)
        .header("Accept", "application/vnd.github+json");
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let meta: GithubReadme = req
        .send()
        .with_context(|| format!("GET {api}"))?
        .error_for_status()
        .with_context(|| format!("README for {owner}/{repo}"))?
        .json()
        .context("parse GitHub README response")?;
    let content = refetch(&meta.download_url)?;
    Ok(Fetched {
        content,
        display: format!("{owner}/{repo}"),
        raw_url: meta.download_url,
    })
}

fn short_url(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .to_string()
}
