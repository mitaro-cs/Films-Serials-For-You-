mod common;
use serde_json::{json, Value};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "настоящие PeerTube и Wikimedia Commons"]
async fn additional_sources_have_playable_media() {
    let dir = tempfile::tempdir().unwrap();
    let (running, host) = common::start(dir.path()).await;
    for provider in ["peertube", "commons"] {
        let result = host.json(&format!("/api/links/search?q=Nosferatu&min=2400&provider={provider}")).await;
        let rows = result.as_array().unwrap();
        assert!(!rows.is_empty(), "{provider}: empty");
        let item = rows
            .iter()
            .find(|v| {
                v["site"] == provider && v["label"].as_str().is_some_and(|s| s.to_lowercase().contains("nosferatu"))
            })
            .unwrap();
        let media = item["url"].as_str().unwrap();
        let response = running
            .app
            .http
            .get(media)
            .header("Range", "bytes=0-16383")
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success(), "{provider}: {}", response.status());
        if media.contains(".m3u8") {
            let body = response.text().await.unwrap();
            assert!(body.starts_with("#EXTM3U"));
        } else {
            let bytes = response.bytes().await.unwrap();
            assert!(bytes.len() > 1024);
            assert!(bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) || bytes.get(4..8) == Some(b"ftyp"));
        }
        println!("{provider}: {} · {} секунд · media confirmed", item["label"], item["duration"]);
    }
    running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "прямой публичный HLS Mux"]
async fn direct_hls_native_quality() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let r = host
        .send(
            "POST",
            "/api/links",
            json!({"url":"https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8","label":"Прямой HLS"}),
        )
        .await;
    assert!(r.status().is_success());
    let id = r.json::<Value>().await.unwrap()["id"].as_i64().unwrap();
    let source = host.json(&format!("/api/links/{id}/stream/source")).await;
    assert_eq!(source["hls"], true);
    let path = format!("{}?height=720", source["url"].as_str().unwrap());
    let r = host.get(&path).await;
    assert!(r.status().is_success());
    let master = r.text().await.unwrap();
    let variants = master.lines().filter(|l| l.starts_with("#EXT-X-STREAM-INF:")).collect::<Vec<_>>();
    assert!(!variants.is_empty());
    assert!(variants.iter().all(|l| l.contains("x720")));
    let uri = master.lines().find(|l| !l.starts_with('#') && !l.is_empty()).unwrap();
    let variant = host.get(&format!("/api/links/{id}/stream/{uri}")).await;
    assert!(variant.status().is_success());
    assert!(variant.text().await.unwrap().contains("#EXTINF"));
    running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "проверка настоящего потока Rutube и TVMaze"]
async fn native_rutube_manifest_segment_and_series_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let response=host.send("POST","/api/links",json!({"url":"https://rutube.ru/video/cc8ffa29410bfca0ae7248bdd16e5c0b/","label":"Немыслимое","thumb":"https://example.com/ignore.jpg"})).await;
    assert!(response.status().is_success());
    let id = response.json::<Value>().await.unwrap()["id"].as_i64().unwrap();
    let source = host.json(&format!("/api/links/{id}/stream/source")).await;
    assert_eq!(source["hls"], true);
    let mut path = source["url"].as_str().unwrap().to_string();
    let master = host.get(&path).await.text().await.unwrap();
    let height = master
        .lines()
        .find_map(|line| {
            line.strip_prefix("#EXT-X-STREAM-INF:")?
                .split(',')
                .find_map(|a| a.strip_prefix("RESOLUTION=")?.split_once('x')?.1.parse::<u32>().ok())
        })
        .expect("источник не объявил разрешения");
    path = format!("{path}?height={height}");
    let selected = host.get(&path).await.text().await.unwrap();
    assert!(selected.contains(&format!("x{height}")));
    assert!(selected
        .lines()
        .filter(|l| l.starts_with("#EXT-X-STREAM-INF:"))
        .all(|l| l.contains(&format!("x{height}"))));
    println!("Native HLS: выбран поток {height}p с сохранением групп озвучки");
    let mut got_segment = false;
    for _ in 0..3 {
        let response = host.get(&path).await;
        assert!(response.status().is_success(), "native proxy {}", response.status());
        let is_playlist =
            response.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|s| s.contains("mpegurl"));
        if !is_playlist {
            let bytes = response.bytes().await.unwrap();
            assert!(bytes.len() > 1024);
            got_segment = true;
            println!("Native Rutube: получен видеосегмент, {} байт", bytes.len());
            if let Ok(frame) = std::env::var("NOVA_LIVE_FRAME") {
                let segment = dir.path().join("native-segment.ts");
                std::fs::write(&segment, &bytes).unwrap();
                let decoded = std::process::Command::new("ffmpeg")
                    .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                    .arg(segment)
                    .args(["-ss", "0.5", "-frames:v", "1"])
                    .arg(&frame)
                    .output()
                    .unwrap();
                assert!(decoded.status.success(), "{}", String::from_utf8_lossy(&decoded.stderr));
                let image = std::fs::read(frame).unwrap();
                assert!(image.len() > 32 && image.starts_with(b"\x89PNG\r\n\x1a\n"));
            }
            break;
        }
        let body = response.text().await.unwrap();
        assert!(body.starts_with("#EXTM3U"));
        let uris = body.lines().filter(|s| !s.is_empty() && !s.starts_with('#')).collect::<Vec<_>>();
        let next =
            if body.contains("#EXTINF") { uris.get(40).or_else(|| uris.first()).copied().unwrap() } else { uris[0] };
        let url = reqwest::Url::parse(&format!("{}{path}", host.base)).unwrap().join(next).unwrap();
        path = format!("{}?{}", url.path(), url.query().unwrap_or_default());
    }
    assert!(got_segment, "не получен видеосегмент после плейлистов");
    let app = &running.app;
    fsfy::discover::upsert(
        app,
        &[fsfy::discover::Ext {
            id: "tt0159206".into(),
            series: true,
            title: "Секс в большом городе".into(),
            original: Some("Sex and the City".into()),
            year: Some(1998),
            rating: None,
            votes: None,
            genres: vec![],
            poster_url: None,
        }],
    )
    .unwrap();
    fsfy::media_meta::series(app, "tt0159206").await.unwrap();
    fsfy::media_meta::episodes(app, "tt0159206", 1).await.unwrap();
    let (art,count):(Option<String>,i64)=app.db.with(|c|c.query_row("SELECT backdrop,(SELECT COUNT(*) FROM ext_episodes WHERE ext_id='tt0159206' AND season=1) FROM ext_titles WHERE id='tt0159206'",[],|r|Ok((r.get(0)?,r.get(1)?)))).unwrap();
    assert!(art.is_some());
    assert_eq!(count, 12);
    println!("TVMaze: фон и 12 серий первого сезона подтверждены");
    running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "проверка настоящего публичного MP4 Internet Archive"]
async fn archive_public_domain_range() {
    let dir = tempfile::tempdir().unwrap();
    let (running, host) = common::start(dir.path()).await;
    let results = host.json("/api/links/search?q=Night%20of%20the%20Living%20Dead&min=600").await;
    assert!(results.as_array().unwrap().iter().any(|v| v["site"] == "archive"), "{results}");
    let client = reqwest::Client::new();
    let meta: Value = client
        .get("https://archive.org/metadata/night-of-the-living-dead-extended-version")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(meta["metadata"]["licenseurl"].as_str().unwrap().contains("creativecommons.org/publicdomain/"));
    let file = meta["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"].as_str().is_some_and(|n| n.ends_with(".mp4")))
        .unwrap();
    let mut url =
        reqwest::Url::parse("https://archive.org/download/night-of-the-living-dead-extended-version/").unwrap();
    url.path_segments_mut().unwrap().pop_if_empty().push(file["name"].as_str().unwrap());
    let response = client.get(url).header("Range", "bytes=0-4095").send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.bytes().await.unwrap().len(), 4096);
    println!("Internet Archive: лицензия Public Domain и Range 206 подтверждены");
    running.shutdown().await;
}
