//! Независимые метаданные и ленивые иллюстрации: сеть не задерживает сохранение карточки.
use crate::{
    auth::Viewer,
    error::{ApiError, ApiResult},
};
use crate::{discover, meta::imdb::Imdb, App};
use axum::extract::{Query, State};
use reqwest::Url;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

pub fn art(url: &str) -> String {
    let mut u = Url::parse("http://localhost/api/discover/art").expect("static URL");
    u.query_pairs_mut().append_pair("url", url);
    format!("/api/discover/art?{}", u.query().unwrap_or_default())
}
pub fn art_allowed(raw: &str) -> bool {
    let Ok(u) = Url::parse(raw) else { return false };
    u.scheme() == "https"
        && u.host_str().is_some_and(|h| {
            ["media-amazon.com", "tmdb.org", "tvmaze.com", "wikimedia.org", "mzstatic.com"]
                .iter()
                .any(|base| h == *base || h.ends_with(&format!(".{base}")))
        })
}
#[derive(Deserialize)]
pub struct ArtQuery {
    url: String,
}
pub async fn image(
    State(app): State<Arc<App>>,
    _v: Viewer,
    Query(q): Query<ArtQuery>,
) -> ApiResult<axum::response::Redirect> {
    if q.url.len() > 4096 || !art_allowed(&q.url) {
        return Err(ApiError::not_found());
    }
    let key = format!("art.cache.{}", crate::util::sha256_hex(q.url.as_bytes()));
    if let Some(name) = app.db.setting(&key).filter(|name| app.images.path(name).is_some_and(|path| path.is_file())) {
        return Ok(axum::response::Redirect::temporary(&format!("/img/{name}")));
    }
    let name = tokio::time::timeout(Duration::from_secs(8), app.images.download(&q.url))
        .await
        .map_err(|_| ApiError::not_found())?
        .map_err(|_| ApiError::not_found())?;
    let _ = app.db.set_setting(&key, &name);
    Ok(axum::response::Redirect::temporary(&format!("/img/{name}")))
}
async fn get(app: &App, url: Url) -> anyhow::Result<Value> {
    Ok(app.http.get(url).timeout(Duration::from_secs(5)).send().await?.error_for_status()?.json().await?)
}
pub async fn tv_show(app: &App, id: &str) -> anyhow::Result<Value> {
    anyhow::ensure!(discover::valid_id(id), "IMDb id");
    get(app, Url::parse_with_params("https://api.tvmaze.com/lookup/shows", &[("imdb", id)])?).await
}
pub async fn series(app: &App, id: &str) -> anyhow::Result<()> {
    let show = tv_show(app, id).await?;
    let image = show["image"]["original"].as_str().map(art);
    let mut background = image.clone();
    if let Some(n) = show["id"].as_i64() {
        if let Ok(images) = get(app, Url::parse(&format!("https://api.tvmaze.com/shows/{n}/images"))?).await {
            background = images
                .as_array()
                .into_iter()
                .flatten()
                .find(|i| i["type"] == "background")
                .and_then(|i| i["resolutions"]["original"]["url"].as_str())
                .map(art)
                .or(background);
        }
    }
    let seasons = if let Some(n) = show["id"].as_i64() {
        get(app, Url::parse(&format!("https://api.tvmaze.com/shows/{n}/seasons"))?)
            .await
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v["number"].as_i64())
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    app.db.with(|c|{
        let raw:Option<String>=c.query_row("SELECT extra FROM ext_titles WHERE id=?",[id],|r|r.get(0))?;
        let mut extra:Value=raw.and_then(|s|serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        if extra["poster"].is_null(){extra["poster"]=json!(image);}
        if !seasons.is_empty(){extra["seasons"]=json!(seasons);}
        extra["tvmaze"]=show["id"].clone();
        c.execute("UPDATE ext_titles SET poster_url=COALESCE(poster_url,?1),backdrop=COALESCE(NULLIF(backdrop,''),?2),runtime=COALESCE(runtime,?3),extra=?4 WHERE id=?5",params![show["image"]["original"].as_str(),background,show["averageRuntime"].as_i64(),extra.to_string(),id])?;Ok(())
    })?;
    Ok(())
}
pub async fn episodes(app: &App, id: &str, season: i32) -> anyhow::Result<()> {
    let show = tv_show(app, id).await?;
    let n = show["id"].as_i64().ok_or_else(|| anyhow::anyhow!("TVMaze id"))?;
    let all = get(app, Url::parse(&format!("https://api.tvmaze.com/shows/{n}/episodes"))?).await?;
    let tags = regex::Regex::new("<[^>]+>")?;
    app.db.tx(|tx|{
        for e in all.as_array().into_iter().flatten().filter(|e|e["season"].as_i64()==Some(season.into())) {
            let Some(ep)=e["number"].as_i64() else{continue};
            let plain=e["summary"].as_str().map(|s|tags.replace_all(s,"").into_owned());
            tx.execute("INSERT INTO ext_episodes(ext_id,season,episode,name,overview,still_url,air_date,runtime,rating) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(ext_id,season,episode) DO UPDATE SET name=COALESCE(ext_episodes.name,excluded.name),overview=COALESCE(ext_episodes.overview,excluded.overview),still_url=COALESCE(ext_episodes.still_url,excluded.still_url),runtime=COALESCE(ext_episodes.runtime,excluded.runtime),air_date=COALESCE(ext_episodes.air_date,excluded.air_date)",params![id,season,ep,e["name"].as_str(),plain,e["image"]["medium"].as_str(),e["airdate"].as_str(),e["runtime"].as_i64(),e["rating"]["average"].as_f64()])?;
        }
        Ok(())
    })?;
    Ok(())
}
pub async fn poster_candidate(app: &App, id: &str) -> anyhow::Result<Option<String>> {
    let (title, original, kind): (String, Option<String>, String) = app.db.with(|c| {
        c.query_row("SELECT title,original,kind FROM ext_titles WHERE id=?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
    })?;
    if kind == "series" {
        if let Ok(show) = tv_show(app, id).await {
            if let Some(url) = show["image"]["original"].as_str() {
                return Ok(Some(url.into()));
            }
        }
    }
    let imdb = Imdb::new(app.http.clone());
    for q in [Some(title.as_str()), original.as_deref()].into_iter().flatten() {
        if let Ok(Ok(found)) =
            tokio::time::timeout(Duration::from_secs(3), imdb.search(q, None, kind == "series")).await
        {
            if let Some(image) = found.into_iter().find(|c| c.id == id).and_then(|c| c.poster) {
                return Ok(Some(image));
            }
        }
    }
    Ok(None)
}

pub async fn show_search(app: &App, q: &str) -> anyhow::Result<Vec<String>> {
    let results = get(app, Url::parse_with_params("https://api.tvmaze.com/search/shows", &[("q", q)])?).await?;
    let mut ids = vec![];
    for row in results.as_array().into_iter().flatten().take(25) {
        let s = &row["show"];
        let Some(id) = s["externals"]["imdb"].as_str().filter(|id| discover::valid_id(id)) else { continue };
        let item = discover::Ext {
            id: id.into(),
            series: true,
            title: s["name"].as_str().unwrap_or(q).into(),
            original: None,
            year: s["premiered"].as_str().and_then(|s| s.get(..4)).and_then(|s| s.parse().ok()),
            rating: None,
            votes: None,
            genres: vec![],
            poster_url: s["image"]["original"].as_str().map(String::from),
        };
        discover::upsert(app, &[item])?;
        ids.push(id.into());
    }
    Ok(ids)
}

pub fn existing_image(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(|s| if s.starts_with('/') { s.into() } else { format!("/img/{s}") })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_proxy_is_limited_to_known_art_hosts() {
        assert!(art_allowed("https://m.media-amazon.com/images/a.jpg"));
        assert!(art_allowed("https://static.tvmaze.com/uploads/a.jpg"));
        for raw in [
            "http://static.tvmaze.com/a.jpg",
            "https://tvmaze.com.evil.example/a.jpg",
            "https://127.0.0.1/a.jpg",
            "file:///C:/Windows/a.jpg",
        ] {
            assert!(!art_allowed(raw));
        }
        let proxy = art("https://static.tvmaze.com/a.jpg?x=1&y=2");
        assert!(proxy.starts_with("/api/discover/art?url=https%3A%2F%2F"));
        assert!(proxy.contains("%26y%3D2"));
    }
}
