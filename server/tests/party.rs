mod common;
use serde_json::{json, Value};

struct Events {
    response: reqwest::Response,
    buffer: String,
}
impl Events {
    async fn event(&mut self, kind: &str) -> Value {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if let Some(end) = self.buffer.find("\n\n") {
                    let packet = self.buffer[..end].to_string();
                    self.buffer.drain(..end + 2);
                    if let Some(data) = packet.lines().find_map(|l| l.strip_prefix("data: ")) {
                        let v: Value = serde_json::from_str(data).unwrap();
                        if v["type"] == kind {
                            return v;
                        }
                    }
                } else {
                    let b = self.response.chunk().await.unwrap().expect("SSE ended");
                    self.buffer.push_str(&String::from_utf8_lossy(&b));
                }
            }
        })
        .await
        .expect("room event did not arrive")
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ten_viewers_receive_ordered_controls_and_rejoin_at_current_position() {
    exercise_viewers(10).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fifty_active_viewers_receive_controls_and_query_without_errors() {
    exercise_viewers(50).await;
}

async fn exercise_viewers(count: usize) {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let media = dir.path().join("range-test.mp4");
    std::fs::write(&media, vec![42u8; 1024 * 1024]).unwrap();
    running
        .app
        .db
        .with(|c| {
            c.execute(
                "INSERT INTO titles(id,kind,key,title,added_at,updated_at) VALUES(9900,'movie','load-test','Load',0,0)",
                [],
            )?;
            c.execute(
                "INSERT INTO files(id,title_id,path,size,mtime,added_at) VALUES(9900,9900,?,1048576,0,0)",
                [media.to_string_lossy().as_ref()],
            )?;
            Ok(())
        })
        .unwrap();
    let link = host
        .send("POST", "/api/links", json!({"url":"https://example.com/party.webm","label":"Party","duration":600}))
        .await
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let room =
        host.send("POST", "/api/rooms", json!({"linkId":link,"position":120})).await.json::<Value>().await.unwrap()
            ["id"]
            .as_str()
            .unwrap()
            .to_owned();
    let mut clients = Vec::new();
    let mut streams = Vec::new();
    for n in 0..count {
        if n > 0 {
            let path = dir.path().join(format!("range-{n}.mp4"));
            std::fs::copy(&media, &path).unwrap();
            running
                .app
                .db
                .with(|c| {
                    c.execute(
                        "INSERT INTO files(id,title_id,path,size,mtime,added_at) VALUES(?,9900,?,1048576,0,0)",
                        rusqlite::params![9900 + n as i64, path.to_string_lossy().as_ref()],
                    )
                })
                .unwrap();
        }
        let login = format!("party{n}");
        assert!(host
            .send(
                "POST",
                "/api/admin/users",
                json!({"login":login,"name":format!("Зритель {n}"),"password":"PartyPass123","role":"viewer"})
            )
            .await
            .status()
            .is_success());
        let mut user = common::Client::new(format!("http://127.0.0.1:{}", running.app.ports.0));
        assert!(user
            .send("POST", "/api/auth/login", json!({"login":login,"password":"PartyPass123"}))
            .await
            .status()
            .is_success());
        let response = user.get(&format!("/api/rooms/{room}/events")).await;
        let mut stream = Events { response, buffer: String::new() };
        stream.event("snapshot").await;
        clients.push(user);
        streams.push(stream);
    }
    assert_eq!(host.json(&format!("/api/rooms/{room}")).await["members"].as_array().unwrap().len(), count);
    let load_seconds: u64 = std::env::var("NOVA_STREAM_LOAD_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let video_load: Vec<_> = clients
        .iter()
        .enumerate()
        .map(|(index, user)| {
            let http = user.http.clone();
            let cookie = user.cookie.clone();
            let url = format!("{}/api/files/{}/raw", user.base, 9900 + index);
            tokio::spawn(async move {
                let mut timings = Vec::new();
                let expected = vec![42u8; 262144];
                let mut clock = tokio::time::interval(std::time::Duration::from_millis(250));
                // Fixed offered bitrate: catch up after scheduler jitter instead of
                // silently reducing the workload by shifting every subsequent tick.
                clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);
                let started = std::time::Instant::now();
                for _ in 0..load_seconds * 4 {
                    clock.tick().await;
                    let tick = std::time::Instant::now();
                    let response = http
                        .get(&url)
                        .header("cookie", &cookie)
                        .header("range", "bytes=0-262143")
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
                    let bytes = response.bytes().await.unwrap();
                    assert_eq!(bytes.len(), 262144);
                    assert_eq!(bytes.as_ref(), expected.as_slice());
                    timings.push(tick.elapsed().as_secs_f64() * 1000.0);
                }
                (timings, started.elapsed().as_secs_f64())
            })
        })
        .collect();
    let started = std::time::Instant::now();
    let durations = futures_util::future::join_all(clients.iter().map(|user| async {
        let mut times = Vec::new();
        for _ in 0..20 {
            let tick = std::time::Instant::now();
            let value = user.json("/api/badges").await;
            assert_eq!(value["unread"], 0);
            times.push(tick.elapsed().as_secs_f64() * 1000.0);
        }
        for _ in 0..8 {
            let response = user
                .http
                .get(format!("{}/api/files/9900/raw", user.base))
                .header("cookie", &user.cookie)
                .header("range", "bytes=0-262143")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
            assert_eq!(response.headers()["content-range"], "bytes 0-262143/1048576");
            let bytes = response.bytes().await.unwrap();
            assert_eq!(bytes.len(), 262144);
            assert!(bytes.iter().all(|b| *b == 42));
        }
        times
    }))
    .await;
    let mut durations: Vec<_> = durations.into_iter().flatten().collect();
    durations.sort_by(f64::total_cmp);
    println!(
        "{count} users, {} requests, {:.2}s, p50 {:.2}ms, p95 {:.2}ms, max {:.2}ms",
        durations.len(),
        started.elapsed().as_secs_f64(),
        durations[durations.len() / 2],
        durations[durations.len() * 95 / 100],
        durations.last().unwrap()
    );
    for n in 0..20usize {
        let position = 120.0 + n as f64;
        let playing = n % 2 == 0;
        let state = clients[n % count]
            .send("POST", &format!("/api/rooms/{room}/state"), json!({"playing":playing,"position":position}))
            .await
            .json::<Value>()
            .await
            .unwrap();
        let events = futures_util::future::join_all(streams.iter_mut().map(|s| s.event("state"))).await;
        for event in events {
            assert_eq!(event["state"]["revision"], state["revision"]);
            assert_eq!(event["state"]["playing"], playing);
            assert_eq!(event["state"]["position"], position);
        }
    }
    // Десять участников нажимают одновременно: сервер задаёт единый порядок,
    // который видят все клиенты, независимо от порядка ответов HTTP.
    let writes = futures_util::future::join_all(clients.iter_mut().enumerate().map(|(n, user)| {
        let endpoint = format!("/api/rooms/{room}/state");
        async move { user.send("POST", &endpoint, json!({"playing":false,"position":200+n})).await }
    }))
    .await;
    let mut ordered = Vec::new();
    for response in writes {
        assert!(response.status().is_success());
        ordered.push(response.json::<Value>().await.unwrap());
    }
    ordered.sort_by_key(|state| state["revision"].as_u64().unwrap());
    for state in &ordered {
        let events = futures_util::future::join_all(streams.iter_mut().map(|s| s.event("state"))).await;
        for event in events {
            assert_eq!(event["state"]["revision"], state["revision"]);
            assert_eq!(event["state"]["position"], state["position"]);
        }
    }
    let reconnect = clients[0].get(&format!("/api/rooms/{room}/events")).await;
    let mut rejoined = Events { response: reconnect, buffer: String::new() };
    let snapshot = rejoined.event("snapshot").await;
    assert_eq!(snapshot["state"]["position"], ordered.last().unwrap()["position"]);
    assert_eq!(snapshot["state"]["revision"], 20 + count);
    let mut segment_times = Vec::new();
    let mut longest = 0.0_f64;
    for task in video_load {
        let (times, elapsed) = task.await.unwrap();
        segment_times.extend(times);
        longest = longest.max(elapsed);
    }
    if !segment_times.is_empty() {
        segment_times.sort_by(f64::total_cmp);
        println!(
            "Sustained {count} streams: {} ranges, {:.1} MiB, {:.2}s, {:.1} Mbps total, range p95 {:.2}ms, max {:.2}ms",
            segment_times.len(),
            segment_times.len() as f64 / 4.0,
            longest,
            segment_times.len() as f64 * 262144.0 * 8.0 / longest / 1_000_000.0,
            segment_times[segment_times.len() * 95 / 100],
            segment_times.last().unwrap()
        );
    }
    drop(rejoined);
    drop(streams);
    running.app.db.with(|c| c.execute("UPDATE users SET disabled=1 WHERE login='party0'", [])).unwrap();
    assert_eq!(clients[0].get("/api/me").await.status(), reqwest::StatusCode::UNAUTHORIZED);
    running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn play_link_does_not_wait_for_remote_thumbnail_and_cloudpub_url_survives_offline() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let started = std::time::Instant::now();
    let response=host.send("POST","/api/links",json!({"url":"https://rutube.ru/video/0123456789abcdef0123456789abcdef/","label":"Ready now","thumb":"https://example.com/slow-thumbnail.jpg"})).await;
    assert!(response.status().is_success());
    assert!(started.elapsed() < std::time::Duration::from_millis(700));
    running.app.db.set_setting("access.mode", "cloudpub").unwrap();
    running.app.db.set_setting("access.cp.url", "https://saved-address.cloudpub.ru").unwrap();
    let view = running.app.access.view(&running.app);
    assert_eq!(view.public_url.as_deref(), Some("https://saved-address.cloudpub.ru"));
    running.shutdown().await;
}
