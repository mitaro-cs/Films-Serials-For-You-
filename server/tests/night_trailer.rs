mod common;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn night_is_local_only_and_trailer_replaces_old_english_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (running, mut host) = common::start(dir.path()).await;
    let mut viewer = common::Client::new(format!("http://127.0.0.1:{}", running.app.ports.0));
    assert!(viewer
        .send("POST", "/api/auth/register", json!({"login":"nightviewer","name":"Зритель","password":"Password123"}))
        .await
        .status()
        .is_success());
    assert_eq!(viewer.send("POST", "/api/host/night", json!({"action":"off"})).await.status(), 404);
    assert_eq!(host.send("POST", "/api/host/night", json!({"action":"shutdown"})).await.status(), 400);
    let result: serde_json::Value =
        host.send("POST", "/api/host/night", json!({"action":"probe"})).await.json().await.unwrap();
    assert_eq!(result["ok"], false); // Headless must not pretend it turned off a screen.
    running.app.db.with(|c|c.execute("INSERT INTO titles(kind,key,title,imdb_id,year,added_at,updated_at) VALUES('movie','godfather','Крёстный отец','tt0068646',1972,0,0)",[])).unwrap();
    running
        .app
        .db
        .set_setting("trailer.v1.tt0068646", &json!({"linkId":-99,"at":fsfy::util::now_ms()}).to_string())
        .unwrap();
    let preview = viewer.json("/api/discover/title/tt0068646/trailer").await;
    let id = preview["linkId"].as_i64().unwrap();
    assert!(id > 0);
    let (url, label): (String, String) = running
        .app
        .db
        .with(|c| c.query_row("SELECT url,label FROM links WHERE id=?", [id], |r| Ok((r.get(0)?, r.get(1)?))))
        .unwrap();
    assert_eq!(url, "https://media.kg-portal.ru/movies/g/godfather/trailers/godfather_dubtrailer_1920.mp4");
    assert!(label.contains("дублированный"));
    assert_eq!(viewer.json("/api/discover/title/tt0068646/trailer").await["linkId"], id);
    running.shutdown().await;
}
