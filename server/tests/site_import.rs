mod common;
use serde_json::json;

#[tokio::test]
#[ignore = "live site search; isolated temporary database"]
async fn selected_sites_search_by_movie_title() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    for provider in ["gidonline", "lordfilm"] {
        let mut url = reqwest::Url::parse("http://localhost/api/links/search").unwrap();
        url.query_pairs_mut()
            .append_pair("q", "Гарри Поттер и философский камень")
            .append_pair("provider", provider)
            .append_pair("min", "2400");
        let response = host.get(&format!("/api/links/search?{}", url.query().unwrap())).await;
        let status = response.status();
        let result: serde_json::Value = response.json().await.unwrap();
        assert!(status.is_success(), "{provider}: {result}");
        let items = result.as_array().unwrap_or_else(|| panic!("{provider}: {result}"));
        assert!(!items.is_empty(), "{provider}: {result}");
        assert!(items.iter().all(|i| i["label"].as_str().unwrap().contains("2001")), "{result}");
        let saved = host.send("POST", "/api/links", json!({"url":items[0]["url"],"label":items[0]["label"]})).await;
        let status = saved.status();
        let saved: serde_json::Value = saved.json().await.unwrap();
        assert!(status.is_success(), "{provider}: {saved}");
        let id = saved["id"].as_i64().unwrap();
        let source = host.get(&format!("/api/links/{id}/stream/source")).await;
        let status = source.status();
        let source: serde_json::Value = source.json().await.unwrap();
        assert!(status.is_success() && source["url"].as_str().is_some(), "{provider} stream: {source}");
        println!("{provider}: {} matching result(s)", items.len());
    }
    running.shutdown().await;
}

#[tokio::test]
#[ignore = "live catalogue; isolated temporary database"]
async fn lordfilm_collection_import_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    running.app.db.set_setting("meta.provider", "off").unwrap();
    let result: serde_json::Value = host
        .send("POST", "/api/host/sites/import", json!({"url":"https://gz.lordfilm5.pro/podborki/"}))
        .await
        .json()
        .await
        .unwrap();
    let count = result["added"].as_i64().unwrap_or(0);
    assert!((1..=20).contains(&count), "{result}");
    println!("Imported {count}; skipped {}", result["skipped"].as_array().unwrap().len());
    running.shutdown().await;
}

#[tokio::test]
async fn importer_is_host_only_and_rejects_private_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let mut viewer = common::Client::new(format!("http://127.0.0.1:{}", running.app.ports.0));
    viewer
        .send("POST", "/api/auth/register", json!({"login":"importviewer","name":"Viewer","password":"Password123"}))
        .await;
    assert_eq!(viewer.send("POST", "/api/host/sites/import", json!({"url":"https://example.org"})).await.status(), 404);
    assert_eq!(host.send("POST", "/api/host/sites/import", json!({"url":"http://127.0.0.1/"})).await.status(), 400);
    running.shutdown().await;
}

#[tokio::test]
#[ignore = "live public site; isolated temporary database"]
async fn gidonline_movie_import_and_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    running.app.db.set_setting("meta.provider", "off").unwrap();
    let body = json!({"url":"https://ag.gidonline.fun/383_9-Garri-Potter-i-Filosofskij-Kamenx/"});
    let result: serde_json::Value =
        host.send("POST", "/api/host/sites/import", body.clone()).await.json().await.unwrap();
    assert_eq!(result["added"], 1, "{result}");
    let titles = host.json("/api/titles").await;
    assert_eq!(titles.as_array().unwrap().len(), 1);
    let link: i64 = running.app.db.with(|c| c.query_row("SELECT id FROM links LIMIT 1", [], |r| r.get(0))).unwrap();
    let source = host.json(&format!("/api/links/{link}/stream/source")).await;
    assert!(source["url"].as_str().is_some(), "{source}");
    let result: serde_json::Value = host.send("POST", "/api/host/sites/import", body).await.json().await.unwrap();
    assert_eq!(result["existing"], 1, "{result}");
    let series =
        host.send("POST", "/api/host/sites/import", json!({"url":"https://ag.gidonline.fun/92109_2-Qrostnaq/"})).await;
    assert_eq!(series.status(), 400);
    assert_eq!(
        host.json("/api/titles").await.as_array().unwrap().len(),
        1,
        "Рекомендации на странице сериала не импортируются вместо него"
    );
    running.shutdown().await;
}
