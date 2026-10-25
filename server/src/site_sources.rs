//! Search adapters for the public catalogue pages selected by the host.
use crate::{
    api::{site_import, stream_link},
    outbound, util,
};
use futures_util::{stream, StreamExt};
use reqwest::Url;
use serde_json::{json, Value};
use std::time::Duration;

pub const HOSTS: &[&str] = &["lordfilm5.pro", "boevikx1.kinoklass.net", "ag.gidonline.fun"];

pub fn known(provider: &str) -> bool {
    matches!(provider, "lordfilm" | "kinoklass" | "gidonline")
}

fn same_site(provider: &str, url: &Url) -> bool {
    let host = url.host_str().unwrap_or("");
    outbound::valid_url(url)
        && match provider {
            "lordfilm" => host == "lordfilm5.pro" || host.ends_with(".lordfilm5.pro"),
            "gidonline" => host == "ag.gidonline.fun",
            "kinoklass" => host == "boevikx1.kinoklass.net",
            _ => false,
        }
}

pub async fn search(provider: &str, query: &str, expected: Option<(&str, Option<i64>)>) -> anyhow::Result<Vec<Value>> {
    let home = match provider {
        "lordfilm" => "https://gz.lordfilm5.pro/podborki/",
        "gidonline" => "https://ag.gidonline.fun/home/",
        "kinoklass" => "https://boevikx1.kinoklass.net/",
        _ => anyhow::bail!("Неизвестный сайт"),
    };
    let (base, html) = stream_link::page_with_url(home).await?;
    anyhow::ensure!(same_site(provider, &base), "Сайт перенаправил на неизвестный домен");
    let (path, fields) = match provider {
        "gidonline" => ("/search/", vec![("search", query)]),
        "lordfilm" => ("/search-result/", vec![("do", "search"), ("subaction", "search"), ("story", query)]),
        // Do not guess endpoints when the site doesn't expose a supported search form.
        _ => anyhow::bail!("Поиск Kinoklass пока недоступен: сайт не предоставил поддерживаемую форму"),
    };
    anyhow::ensure!(html.contains(path), "На сайте изменилась форма поиска");
    let (base, html) = stream_link::form_page(base.join(path)?.as_str(), &fields).await?;
    anyhow::ensure!(same_site(provider, &base), "Поиск перенаправил на неизвестный домен");
    let urls = site_import::catalogue(&html, &base);
    let mut pages = stream::iter(urls.into_iter().take(8).map(|url| async move {
        let html = tokio::time::timeout(Duration::from_secs(6), stream_link::page(url.as_str())).await.ok()?.ok()?;
        let (title, year) = site_import::movie(&html)?;
        if let Some((name, wanted_year)) = expected {
            if util::norm(&title) != util::norm(name) || wanted_year.is_some_and(|y| y != year) {
                return None;
            }
        } else if !crate::video_search::matches(&format!("{title} ({year})"), query, None, None, None, false) {
            return None;
        }
        if stream_link::find_iframes(&html, &url).is_empty() && stream_link::find_media(&html).is_none() {
            return None;
        }
        Some(json!({"site":provider,"url":url.as_str(),"label":format!("{title} ({year})"),
            "duration":null,"thumbUrl":null,"thumb":null,"author":provider}))
    }))
    .buffer_unordered(2);
    let mut items = Vec::new();
    while let Some(item) = pages.next().await {
        if let Some(item) = item {
            items.push(item);
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirrors_stay_within_selected_site() {
        assert!(same_site("lordfilm", &Url::parse("https://sa.lordfilm5.pro/search-result/").unwrap()));
        for url in ["https://lordfilm5.pro.evil.org/", "http://127.0.0.1/", "https://evil.org/"] {
            assert!(!same_site("lordfilm", &Url::parse(url).unwrap()));
        }
    }
}
