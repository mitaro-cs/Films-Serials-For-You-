//! Automatic movie discovery on the built-in LordFilm source.
use reqwest::Url;
use serde_json::{json, Value};

pub const HOST: &str = "sd.lordfilm5.pro";
const BASE: &str = "https://sd.lordfilm5.pro/";

pub fn movie_url(u: &Url) -> bool {
    source_url(u) && (u.path().starts_with("/filmy/") || u.path().starts_with("/mult/"))
}

fn source_url(u: &Url) -> bool {
    u.scheme() == "https"
        && u.host_str() == Some(HOST)
        && u.port().is_none()
        && u.username().is_empty()
        && u.password().is_none()
        && u.path().ends_with(".html")
}

pub fn series_url(u: &Url) -> bool {
    source_url(u) && u.path().starts_with("/serialy/")
}

/// The fragment identifies an episode locally; it is never sent to the source server.
pub fn episode_target(u: &Url) -> Option<(i64, i64)> {
    if !series_url(u) {
        return None;
    }
    let fragment = u.fragment()?;
    let (season, episode) = fragment.strip_prefix("nova-season=")?.split_once("&episode=")?;
    let (season, episode) = (season.parse::<i64>().ok()?, episode.parse::<i64>().ok()?);
    ((0..=1000).contains(&season) && (1..=10000).contains(&episode)).then_some((season, episode))
}

fn decode(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&#039;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
}

fn parse(html: &str, title: &str, year: Option<i64>, series: bool) -> Vec<Value> {
    let base = Url::parse(BASE).expect("source URL");
    let anchors = regex::Regex::new(r#"(?s)<a\b[^>]*class="th-in[^"\n]*"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap();
    let names = regex::Regex::new(r#"<div class="th-title">([^<]+)</div>"#).unwrap();
    let years = regex::Regex::new(r#"<div class="th-series">\s*((?:19|20)\d{2})\s*</div>"#).unwrap();
    let images = regex::Regex::new(r#"<img[^>]+src="([^"]+)""#).unwrap();
    anchors
        .captures_iter(html)
        .filter_map(|c| {
            let url = base.join(&decode(&c[1])).ok()?;
            if !(if series { series_url(&url) } else { movie_url(&url) }) {
                return None;
            }
            let body = &c[2];
            let name = decode(&names.captures(body)?[1]);
            let found_year = years.captures(body).and_then(|c| c[1].parse::<i64>().ok());
            // A sequel or a remake must never silently become this film's play button.
            if crate::util::norm(&name) != crate::util::norm(title) || year.is_some_and(|y| found_year != Some(y)) {
                return None;
            }
            let thumb = images
                .captures(body)
                .and_then(|c| base.join(&decode(&c[1])).ok())
                .filter(crate::outbound::valid_url)
                .map(|u| u.to_string());
            let label = found_year.map_or(name.clone(), |y| format!("{name} ({y})"));
            Some(json!({"site":"lordfilm", "url":url.as_str(), "label":label,
            "thumbUrl":thumb, "duration":null, "author":"LordFilm"}))
        })
        .take(8)
        .collect()
}

pub async fn search(title: &str, year: Option<i64>) -> Option<Vec<Value>> {
    let url = Url::parse_with_params(
        &format!("{BASE}index.php"),
        &[("do", "search"), ("subaction", "search"), ("story", title)],
    )
    .ok()?;
    let html = crate::api::stream_link::page(url.as_str()).await.ok()?;
    Some(parse(&html, title, year, false))
}

fn episode_media(html: &str, season: i64, episode: i64) -> Option<(String, Option<i64>)> {
    let re = regex::Regex::new(r#"\bseasons\s*:\s*(\[)"#).ok()?;
    for capture in re.captures_iter(html) {
        let start = capture.get(1)?.start();
        let Some(Ok(seasons)) = serde_json::Deserializer::from_str(&html[start..]).into_iter::<Value>().next() else {
            continue;
        };
        let number = |v: &Value| v.as_i64().or_else(|| v.as_str()?.parse::<i64>().ok());
        let Some(s) = seasons.as_array()?.iter().find(|s| number(&s["season"]) == Some(season)) else { continue };
        if s["blocked"] == true {
            continue;
        }
        let Some(e) = s["episodes"].as_array()?.iter().find(|e| number(&e["episode"]) == Some(episode)) else {
            continue;
        };
        if e["blocked"] == true {
            continue;
        }
        let Some(url) = e["hls"].as_str().and_then(|u| Url::parse(u).ok()) else { continue };
        if crate::outbound::valid_url(&url) && url.path().ends_with(".m3u8") {
            return Some((url.into(), e["duration"].as_i64().filter(|d| *d > 0)));
        }
    }
    None
}

pub async fn episode_stream(url: &Url) -> anyhow::Result<(String, Option<i64>)> {
    let (season, episode) = episode_target(url).ok_or_else(|| anyhow::anyhow!("Не выбрана серия LordFilm"))?;
    let mut page_url = url.clone();
    page_url.set_fragment(None);
    let (base, html) = crate::api::stream_link::page_with_url(page_url.as_str()).await?;
    // Only parse the structured playlist of the observed provider. Never pick an arbitrary
    // HLS URL from a series page: that could silently play a different episode.
    let frames = crate::api::stream_link::find_iframes(&html, &base);
    for frame in frames.iter().filter(|u| u.scheme() == "https" && u.host_str() == Some("api.nextembed.ws")) {
        let Ok(html) = crate::api::stream_link::page(frame.as_str()).await else { continue };
        if let Some(media) = episode_media(&html, season, episode) {
            return Ok(media);
        }
    }
    anyhow::bail!("LordFilm не предоставил выбранную серию: сезон {season}, серия {episode}")
}

pub async fn search_episode(title: &str, year: Option<i64>, season: i64, episode: i64) -> Option<Vec<Value>> {
    let url = Url::parse_with_params(
        &format!("{BASE}index.php"),
        &[("do", "search"), ("subaction", "search"), ("story", title)],
    )
    .ok()?;
    let html = crate::api::stream_link::page(url.as_str()).await.ok()?;
    let mut found = Vec::new();
    for mut item in parse(&html, title, year, true).into_iter().take(2) {
        let mut url = Url::parse(item["url"].as_str()?).ok()?;
        url.set_fragment(Some(&format!("nova-season={season}&episode={episode}")));
        if let Ok((_, duration)) = episode_stream(&url).await {
            item["url"] = json!(url.as_str());
            item["label"] = json!(format!("{} S{season:02}E{episode:02}", item["label"].as_str()?));
            item["duration"] = json!(duration);
            found.push(item);
        }
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_movie_and_year_without_sequels_or_foreign_links() {
        let item = |url: &str, name: &str, year: i64| {
            format!(
                r#"<a class="th-in with-mask" href="{url}"><img src="/poster.jpg"><div class="th-title">{name}</div><div class="th-series">{year}</div></a>"#
            )
        };
        let html = [
            item("/filmy/1.html", "Дюна", 2021),
            item("/filmy/2.html", "Дюна", 1984),
            item("/filmy/3.html", "Дюна: Часть вторая", 2021),
            item("/serialy/4.html", "Дюна", 2021),
            item("https://evil.example/filmy/5.html", "Дюна", 2021),
        ]
        .join("");
        let results = parse(&html, "Дюна", Some(2021), false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["url"], format!("{BASE}filmy/1.html"));
        assert!(results[0]["duration"].is_null());
        assert!(parse(&html, "Дюна", Some(2020), false).is_empty());
        assert!(!movie_url(&Url::parse("https://sd.lordfilm5.pro.evil.example/filmy/1.html").unwrap()));
        assert!(!movie_url(&Url::parse("https://user@sd.lordfilm5.pro/filmy/1.html").unwrap()));
    }

    #[test]
    fn selects_exact_episode_and_rejects_missing_or_blocked() {
        let html = r#"playlist: {seasons:[{"season":1,"episodes":[{"episode":"1","hls":"https://cdn.example/one.m3u8"},{"episode":"2","hls":"https://cdn.example/two.m3u8","duration":1200},{"episode":"3","hls":"https://cdn.example/three.m3u8","blocked":true}]},{"season":2,"blocked":true,"episodes":[{"episode":"1","hls":"https://cdn.example/other.m3u8"}]}]}, other: true"#;
        assert_eq!(episode_media(html, 1, 2), Some(("https://cdn.example/two.m3u8".into(), Some(1200))));
        for (s, e) in [(1, 3), (1, 10), (2, 1), (3, 1)] {
            assert!(episode_media(html, s, e).is_none());
        }
        assert!(episode_media("https://cdn.example/random.m3u8", 1, 1).is_none());
        let url = Url::parse(&format!("{BASE}serialy/1.html#nova-season=1&episode=2")).unwrap();
        assert_eq!(episode_target(&url), Some((1, 2)));
        assert!(
            episode_target(&Url::parse(&format!("{BASE}serialy/1.html#nova-season=1&episode=0")).unwrap()).is_none()
        );
        assert!(episode_target(&Url::parse(&format!("{BASE}filmy/1.html#nova-season=1&episode=2")).unwrap()).is_none());
    }
}
