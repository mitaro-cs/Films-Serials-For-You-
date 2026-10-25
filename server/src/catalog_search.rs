//! Поиск по IMDb, русской Википедии, Викиданным и сохранённому каталогу.
//! JSON результатов не ждёт скачивания картинок; русские названия имеют отдельный кэш.
use crate::{
    discover::{self, Ext},
    meta::imdb::Imdb,
    App,
};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Duration};

async fn get(app: &App, host: &str, args: &[(&str, &str)]) -> anyhow::Result<Value> {
    Ok(app
        .http
        .get(host)
        .query(args)
        .header("user-agent", "Nova/1.0 (home cinema)")
        .timeout(Duration::from_secs(4))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn entities(app: &App, ids: &str) -> anyhow::Result<Value> {
    get(
        app,
        "https://www.wikidata.org/w/api.php",
        &[
            ("action", "wbgetentities"),
            ("ids", ids),
            ("props", "labels|claims|descriptions|sitelinks"),
            ("languages", "ru|en"),
            ("format", "json"),
        ],
    )
    .await
}
fn claim<'a>(e: &'a Value, p: &str) -> impl Iterator<Item = &'a Value> {
    e["claims"][p].as_array().into_iter().flatten().map(|c| &c["mainsnak"]["datavalue"]["value"])
}
fn label(e: &Value) -> Option<&str> {
    e["labels"]["ru"]["value"].as_str().or(e["labels"]["en"]["value"].as_str())
}

pub async fn wiki_search(app: &App, query: &str, people: bool) -> anyhow::Result<Vec<Value>> {
    let wiki_args = [
        ("action", "query"),
        ("generator", "search"),
        ("gsrsearch", query),
        ("gsrnamespace", "0"),
        ("gsrlimit", "24"),
        ("prop", "pageprops|pageimages"),
        ("piprop", "thumbnail"),
        ("pithumbsize", "342"),
        ("formatversion", "2"),
        ("format", "json"),
    ];
    let name_args =
        [("action", "wbsearchentities"), ("search", query), ("language", "ru"), ("limit", "20"), ("format", "json")];
    let (wiki, names) = tokio::join!(
        get(app, "https://ru.wikipedia.org/w/api.php", &wiki_args),
        get(app, "https://www.wikidata.org/w/api.php", &name_args)
    );
    let mut ids = Vec::<String>::new();
    let mut images = HashMap::<String, String>::new();
    if let Ok(w) = wiki {
        for p in w["query"]["pages"].as_array().into_iter().flatten() {
            if let Some(id) = p["pageprops"]["wikibase_item"].as_str() {
                ids.push(id.into());
                if let Some(image) = p["thumbnail"]["source"].as_str() {
                    images.insert(id.into(), image.into());
                }
            }
        }
    }
    if let Ok(n) = names {
        ids.extend(n["search"].as_array().into_iter().flatten().filter_map(|v| v["id"].as_str().map(String::from)));
    }
    ids.sort();
    ids.dedup();
    ids.truncate(45);
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let response = entities(app, &ids.join("|")).await?;
    let mut out = vec![];
    for (qid, e) in response["entities"].as_object().into_iter().flatten() {
        let Some(id) = claim(e, "P345").filter_map(Value::as_str).find(|s| {
            if people {
                s.starts_with("nm")
            } else {
                discover::valid_id(s)
            }
        }) else {
            continue;
        };
        let Some(title) = label(e) else { continue };
        if !people && claim(e, "P31").any(|v| matches!(v["id"].as_str(), Some("Q7889" | "Q21191270" | "Q1983062"))) {
            continue;
        }
        let series =
            claim(e, "P31").any(|v| matches!(v["id"].as_str(), Some("Q5398426" | "Q15416" | "Q581714" | "Q7725310")));
        let year = claim(e, "P577")
            .filter_map(|v| v["time"].as_str())
            .filter_map(|s| s.trim_start_matches('+').get(..4)?.parse::<i32>().ok())
            .min();
        let data = json!({"qid":qid,"wiki":e["sitelinks"]["ruwiki"]["title"],"description":e["descriptions"]["ru"]["value"],"image":images.get(qid)});
        if !people {
            let item = Ext {
                id: id.into(),
                series,
                title: title.into(),
                original: e["labels"]["en"]["value"].as_str().map(String::from),
                year,
                rating: None,
                votes: None,
                genres: vec![],
                poster_url: images.get(qid).cloned(),
            };
            discover::upsert(app, &[item])?;
            if let Some(ru) = e["labels"]["ru"]["value"].as_str() {
                app.db.with(|c|c.execute("INSERT INTO title_localizations VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,data=excluded.data,at=excluded.at",
                    params![id,ru,data.to_string(),crate::util::now_ms()]))?;
            }
        }
        out.push(
            json!({"id":id,"name":title,"role":"actor","description":e["descriptions"]["ru"]["value"],"photo":null}),
        );
    }
    Ok(out)
}

pub async fn localize(app: &App, ids: &[String]) -> anyhow::Result<()> {
    let ids = ids
        .iter()
        .filter(|id| discover::valid_id(id))
        .take(65)
        .map(|i| format!("\"{i}\""))
        .collect::<Vec<_>>()
        .join(" ");
    if ids.is_empty() {
        return Ok(());
    }
    let query = format!(
        r#"SELECT ?imdb (SAMPLE(?ru) AS ?title) (SAMPLE(?articleName) AS ?wiki) (SAMPLE(?item) AS ?qid)
      (GROUP_CONCAT(DISTINCT ?genreName;separator="|") AS ?genres) WHERE {{ VALUES ?imdb {{ {ids} }}
      ?item wdt:P345 ?imdb. OPTIONAL {{ ?item rdfs:label ?ru FILTER(LANG(?ru)="ru") }}
      OPTIONAL {{ ?article schema:about ?item; schema:isPartOf <https://ru.wikipedia.org/>; schema:name ?articleName }}
      OPTIONAL {{ ?item wdt:P136 ?genre. ?genre rdfs:label ?genreName FILTER(LANG(?genreName)="ru") }}
    }} GROUP BY ?imdb"#
    );
    let v = get(app, "https://query.wikidata.org/sparql", &[("format", "json"), ("query", &query)]).await?;
    for b in v["results"]["bindings"].as_array().into_iter().flatten() {
        let (Some(id), Some(title)) = (b["imdb"]["value"].as_str(), b["title"]["value"].as_str()) else { continue };
        let data =
            json!({"wiki":b["wiki"]["value"],"qid":b["qid"]["value"].as_str().and_then(|s|s.rsplit('/').next())});
        app.db.with(|c|{
            c.execute("INSERT INTO title_localizations VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,data=json_patch(title_localizations.data,excluded.data),at=excluded.at",
                params![id,title,data.to_string(),crate::util::now_ms()])?;
            let genres=b["genres"]["value"].as_str().unwrap_or("").split('|').filter(|s|!s.is_empty()).collect::<Vec<_>>();
            if !genres.is_empty() {c.execute("UPDATE ext_titles SET genres=? WHERE id=?",params![json!(crate::meta::genres_ru(&genres)).to_string(),id])?;}
            Ok(())
        })?;
    }
    Ok(())
}

pub async fn search(app: &Arc<App>, query: &str) -> anyhow::Result<Vec<String>> {
    let key = crate::util::norm(query);
    let fresh: bool = app.db.with(|c| {
        c.query_row(
            "SELECT EXISTS(SELECT 1 FROM catalog_search_cache WHERE query=? AND at>?)",
            params![key, crate::util::now_ms() - 6 * 3_600_000],
            |r| r.get(0),
        )
    })?;
    if fresh {
        return search_full(app, query).await;
    }
    let known:Vec<(String,String)>=app.db.with(|c|{let mut st=c.prepare("SELECT e.id,COALESCE(l.title,'')||' '||e.title||' '||COALESCE(e.original,'') FROM ext_titles e LEFT JOIN title_localizations l ON l.id=e.id LIMIT 30000")?;let rows=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?;rows.collect()})?;
    let words = key.split_whitespace().collect::<Vec<_>>();
    let known = known
        .into_iter()
        .filter(|(id, title)| id == query || words.iter().all(|w| crate::util::norm(title).contains(w)))
        .map(|(id, _)| id)
        .take(80)
        .collect::<Vec<_>>();
    if known.is_empty() {
        return search_full(app, query).await;
    }
    let job = format!("search:{key}");
    if app.detail_jobs.lock().insert(job.clone()) {
        let app = app.clone();
        let q = query.to_string();
        tokio::spawn(async move {
            let _ = search_full(&app, &q).await;
            app.detail_jobs.lock().remove(&job);
        });
    }
    Ok(known)
}

async fn search_full(app: &Arc<App>, query: &str) -> anyhow::Result<Vec<String>> {
    let key = crate::util::norm(query);
    let cached: Option<String> = app.db.with(|c| {
        c.query_row(
            "SELECT ids FROM catalog_search_cache WHERE query=? AND at>?",
            params![key, crate::util::now_ms() - 6 * 3_600_000],
            |r| r.get(0),
        )
        .optional()
    })?;
    if let Some(c) = cached {
        return Ok(serde_json::from_str(&c)?);
    }
    let imdb = Imdb::new(app.http.clone());
    let (suggest, wiki, television) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(4), imdb.search(query, None, false)),
        wiki_search(app, query, false),
        crate::media_meta::show_search(app, query)
    );
    let mut ids = vec![];
    if let Ok(Ok(found)) = suggest {
        let items = found
            .into_iter()
            .filter(|c| discover::valid_id(&c.id))
            .map(|c| Ext {
                id: c.id,
                series: c.series,
                title: c.title,
                original: c.original,
                year: c.year,
                rating: None,
                votes: None,
                genres: vec![],
                poster_url: c.poster,
            })
            .collect::<Vec<_>>();
        discover::upsert(app, &items)?;
        ids.extend(items.into_iter().map(|i| i.id));
    }
    if let Ok(wiki) = wiki {
        ids.extend(wiki.into_iter().filter_map(|v| v["id"].as_str().map(String::from)));
    }
    if let Ok(television) = television {
        ids.extend(television);
    }
    // Сохранённые данные остаются доступны при блокировке внешних API, включая оригинальные названия.
    let known:Vec<(String,String)>=app.db.with(|c| {
        let mut st=c.prepare("SELECT e.id,COALESCE(l.title,'')||' '||e.title||' '||COALESCE(e.original,'')||' '||COALESCE(e.year,'') FROM ext_titles e LEFT JOIN title_localizations l ON l.id=e.id LIMIT 30000")?;
        let rows=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?; rows.collect()
    })?;
    let words = key.split_whitespace().collect::<Vec<_>>();
    ids.extend(
        known
            .into_iter()
            .filter(|(id, title)| id == query || words.iter().all(|w| crate::util::norm(title).contains(w)))
            .map(|(id, _)| id),
    );
    if discover::valid_id(query) && !ids.iter().any(|id| id == query) {
        let _ = tokio::time::timeout(Duration::from_secs(5), discover::detail(app, query)).await;
        if app.db.with(|c| {
            c.query_row("SELECT EXISTS(SELECT 1 FROM ext_titles WHERE id=?)", [query], |r| r.get::<_, bool>(0))
        })? {
            ids.insert(0, query.into());
        }
    }
    let mut seen = std::collections::HashSet::new();
    ids.retain(|i| seen.insert(i.clone()));
    ids.truncate(80);
    let _ = localize(app, &ids).await;
    app.posters_kick.notify_one();
    if !ids.is_empty() {
        app.db.with(|c|c.execute("INSERT INTO catalog_search_cache VALUES(?,?,?) ON CONFLICT(query) DO UPDATE SET ids=excluded.ids,at=excluded.at",params![key,json!(ids).to_string(),crate::util::now_ms()]))?;
    }
    Ok(ids)
}

/// Заполнить основные данные русской статьёй, если IMDb блокирует запросы. Никогда не затирает имеющиеся кадры.
pub async fn fallback_detail(app: &Arc<App>, id: &str) -> anyhow::Result<()> {
    localize(app, &[id.into()]).await?;
    let raw: String =
        app.db.with(|c| c.query_row("SELECT data FROM title_localizations WHERE id=?", [id], |r| r.get(0)))?;
    let data: Value = serde_json::from_str(&raw)?;
    let title = data["wiki"].as_str().ok_or_else(|| anyhow::anyhow!("Нет русской статьи"))?;
    let mut url = reqwest::Url::parse("https://ru.wikipedia.org/api/rest_v1/page/summary/")?;
    url.path_segments_mut().map_err(|_| anyhow::anyhow!("URL"))?.pop_if_empty().push(title);
    let summary: Value =
        app.http.get(url).timeout(Duration::from_secs(5)).send().await?.error_for_status()?.json().await?;
    let overview = summary["extract"].as_str().map(crate::ruwiki::trim);
    let qid = data["qid"].as_str().filter(|s| s.starts_with('Q') && s[1..].bytes().all(|b| b.is_ascii_digit()));
    let mut people = vec![];
    let mut countries = vec![];
    let mut languages = vec![];
    let mut runtime = None;
    if let Some(qid) = qid {
        if let Ok(entity) = entities(app, qid).await {
            let e = &entity["entities"][qid];
            runtime = claim(e, "P2047")
                .filter_map(|v| v["amount"].as_str())
                .find_map(|s| s.parse::<f64>().ok())
                .map(|m| m.round() as i64);
            let mut related = vec![];
            for (p, job, limit) in [
                ("P57", "director", 3),
                ("P58", "writer", 3),
                ("P161", "actor", 12),
                ("P495", "country", 6),
                ("P364", "language", 6),
            ] {
                related.extend(claim(e, p).filter_map(|v| v["id"].as_str()).take(limit).map(|i| (i.to_string(), job)));
            }
            if !related.is_empty() {
                if let Ok(all) = entities(app, &related.iter().map(|r| r.0.clone()).collect::<Vec<_>>().join("|")).await
                {
                    for (rid, job) in related {
                        let p = &all["entities"][&rid];
                        let Some(name) = label(p) else { continue };
                        match job {"country"=>countries.push(name.to_string()),"language"=>languages.push(name.to_string()),_=>people.push(json!({"name":name,"job":job,"role":null,"photo":null,"id":claim(p,"P345").find_map(Value::as_str)}))}
                    }
                }
            }
        }
    }
    app.db.with(|c|{
        let raw:Option<String>=c.query_row("SELECT extra FROM ext_titles WHERE id=?",[id],|r|r.get(0))?;
        let mut extra:Value=raw.and_then(|s|serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        if extra["people"].as_array().is_none_or(|p|p.is_empty()) {extra["people"]=json!(people);}
        if extra["countries"].as_array().is_none_or(|p|p.is_empty()) {extra["countries"]=json!(countries);}
        if extra["languages"].as_array().is_none_or(|p|p.is_empty()) {extra["languages"]=json!(languages);}
        c.execute("UPDATE ext_titles SET overview=COALESCE(?1,overview),runtime=COALESCE(runtime,?2),extra=?3,detailed=MAX(detailed,1) WHERE id=?4",params![overview,runtime,extra.to_string(),id])?;
        Ok(())
    })?;
    Ok(())
}

pub fn genre_ru(g: &str) -> String {
    crate::meta::genre_ru(g)
}
