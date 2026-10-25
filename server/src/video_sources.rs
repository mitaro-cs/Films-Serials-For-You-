use crate::{outbound, App};
use reqwest::Url;
use serde_json::{json, Value};
use std::time::Duration;

async fn get(_app: &App, url: Url) -> Option<Value> {
    if !outbound::valid_url(&url) {
        return None;
    }
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = CLIENT
        .get_or_init(|| outbound::builder().timeout(Duration::from_secs(4)).build().expect("public video client"));
    let r = client
        .get(url)
        .header("User-Agent", "Nova/1.0 (home cinema; public media search)")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let data = outbound::limited(r, 2 * 1024 * 1024).await.ok()?;
    serde_json::from_slice(&data).ok()
}

pub async fn commons(app: &App, title: &str) -> Option<Vec<Value>> {
    let query = format!("intitle:\"{}\" filetype:video", title.replace('"', " "));
    let url = Url::parse_with_params(
        "https://commons.wikimedia.org/w/api.php",
        &[
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("generator", "search"),
            ("gsrnamespace", "6"),
            ("gsrsearch", query.as_str()),
            ("gsrlimit", "8"),
            ("prop", "videoinfo"),
            ("viprop", "url|size|derivatives"),
            ("viurlwidth", "320"),
        ],
    )
    .ok()?;
    let data = get(app, url).await?;
    Some(data["query"]["pages"].as_array().map(|pages| pages.iter().filter_map(|p| {
        let v=p["videoinfo"].as_array()?.first()?;
        let original=v["url"].as_str()?;
        let url=if original.contains(".webm")||original.contains(".mp4") {original} else {
            v["derivatives"].as_array()?.iter().find_map(|d|d["src"].as_str().filter(|u|u.contains(".webm")))?
        };
        let parsed=Url::parse(url).ok()?;
        if parsed.host_str()!=Some("upload.wikimedia.org") { return None; }
        let label=p["title"].as_str()?.trim_start_matches("File:");
        let label=label.rsplit_once('.').map(|(name,_)|name).unwrap_or(label).replace('_'," ");
        if !crate::video_search::matches(&label,title,None,None,None,false) {return None;}
        Some(json!({"site":"commons","url":url,"label":label,"duration":v["duration"].as_f64().map(|v|v.round() as i64),"thumbUrl":v["thumburl"],"author":"Wikimedia Commons","sourceUrl":v["descriptionurl"]}))
    }).collect()).unwrap_or_default())
}

pub async fn peertube(app: &App, title: &str, min: i64) -> Option<Vec<Value>> {
    let url = Url::parse_with_params(
        "https://sepiasearch.org/api/v1/search/videos",
        &[
            ("search", title),
            ("count", "12"),
            ("nsfw", "false"),
            ("categoryOneOf", "2"),
            ("durationMin", &min.max(0).to_string()),
        ],
    )
    .ok()?;
    let data = get(app, url).await?;
    let rows = data["data"].as_array()?;
    let tasks=rows.iter().filter(|v|v["duration"].as_i64().unwrap_or(0)>=min).take(4).map(|row| async move {
        let landing=Url::parse(row["url"].as_str()?).ok()?;
        if !outbound::valid_url(&landing) || landing.scheme()!="https" { return None; }
        let uuid=row["uuid"].as_str()?;
        if !uuid.bytes().all(|b|b.is_ascii_hexdigit()||b==b'-') { return None; }
        let detail=get(app,landing.join(&format!("/api/v1/videos/{uuid}")).ok()?).await?;
        let hls=detail["streamingPlaylists"].as_array().and_then(|p|p.iter().find_map(|p|p["playlistUrl"].as_str()));
        let file=detail["files"].as_array().and_then(|p|p.iter().filter(|p|p["fileUrl"].as_str().is_some_and(|u|u.contains(".mp4"))).max_by_key(|p|p["resolution"]["id"].as_i64().unwrap_or(0))).and_then(|p|p["fileUrl"].as_str());
        let media=hls.or(file)?;
        if !outbound::valid_url(&Url::parse(media).ok()?) { return None; }
        Some(json!({"site":"peertube","url":media,"label":row["name"],"duration":row["duration"],"thumbUrl":row["thumbnailUrl"],"author":format!("PeerTube · {}",landing.host_str().unwrap_or_default()),"sourceUrl":row["url"]}))
    });
    Some(futures_util::future::join_all(tasks).await.into_iter().flatten().collect())
}
