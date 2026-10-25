//! Совместный просмотр: комната на фильм или серию с сервиса. У всех в комнате одно место в видео -
//! пауза, перемотка и следующая серия у всех сразу; чат и реакции поверх видео. Комнаты живут в
//! памяти (перезапуск сервера их закрывает), пустая комната закрывается через 10 минут.
//!
//! Связь - поток событий (SSE) от сервера и обычные запросы от зрителя: проходит через туннели,
//! где WebSocket может не пройти.

use crate::util::now_ms;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Пустая комната (никто не подключён) живёт столько.
const IDLE_MS: i64 = 10 * 60_000;
/// Сообщений чата в памяти комнаты - для тех, кто зашёл позже.
const CHAT_KEEP: usize = 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub revision: u64,
    pub file_id: i64,
    /// Видео по ссылке (фильм из Обзора: Rutube, VK, свой сайт) - тогда `file_id` = 0.
    pub link_id: Option<i64>,
    pub playing: bool,
    /// Место в секундах на момент `at` (мс Unix); при `playing` - идёт дальше.
    pub position: f64,
    pub at: i64,
    /// Кто последним управлял (имя).
    pub by: String,
}

pub struct Room {
    pub id: String,
    pub owner: i64,
    pub title_id: i64,
    pub state: State,
    /// Кто подключён: профиль → (карточка человека, сколько вкладок).
    pub members: HashMap<i64, (Value, usize)>,
    pub chat: Vec<Value>,
    /// Кто в голосовом чате (браузеры связаны напрямую - WebRTC; через нас - только сигналы).
    pub voice: std::collections::BTreeSet<i64>,
    pub empty_since: Option<i64>,
    pub tx: broadcast::Sender<String>,
}

impl Room {
    pub fn members_json(&self) -> Vec<Value> {
        let mut m: Vec<&Value> = self.members.values().map(|(p, _)| p).collect();
        m.sort_by_key(|p| p["id"].as_i64());
        m.into_iter().cloned().collect()
    }

    /// Состояние на сейчас: при «смотрим» место ушло вперёд с момента последней команды (зашедший
    /// позже иначе начинал бы с места той команды - минуты назад).
    pub fn current(&self) -> State {
        let mut s = self.state.clone();
        if s.playing {
            let now = now_ms();
            s.position += (now - s.at).max(0) as f64 / 1000.0;
            s.at = now;
        }
        s
    }

    pub fn snapshot(&self) -> Value {
        json!({
            "type": "snapshot", "id": self.id, "ownerId": self.owner, "titleId": self.title_id,
            "state": self.current(), "members": self.members_json(), "chat": self.chat, "voice": self.voice,
        })
    }

    fn send(&self, v: Value) {
        let _ = self.tx.send(v.to_string());
    }
}

#[derive(Default, Clone)]
pub struct Rooms(Arc<Mutex<HashMap<String, Room>>>);

impl Rooms {
    pub fn create(
        &self,
        owner: i64,
        title_id: i64,
        file_id: i64,
        link_id: Option<i64>,
        position: f64,
        by: String,
    ) -> String {
        let mut map = self.0.lock();
        let id = loop {
            let id = crate::util::human_code(8).to_lowercase();
            if !map.contains_key(&id) {
                break id;
            }
        };
        let (tx, _) = broadcast::channel(64);
        map.insert(
            id.clone(),
            Room {
                id: id.clone(),
                owner,
                title_id,
                state: State { revision: 0, file_id, link_id, playing: false, position, at: now_ms(), by },
                members: HashMap::new(),
                voice: Default::default(),
                chat: Vec::new(),
                empty_since: Some(now_ms()),
                tx,
            },
        );
        id
    }

    pub fn with<T>(&self, id: &str, f: impl FnOnce(&mut Room) -> T) -> Option<T> {
        self.0.lock().get_mut(id).map(f)
    }

    /// Вошёл (ещё одна вкладка): подписка на события и снимок комнаты.
    pub fn join(&self, id: &str, profile: i64, person: Value) -> Option<(broadcast::Receiver<String>, Value)> {
        self.with(id, |r| {
            let e = r.members.entry(profile).or_insert((person, 0));
            e.1 += 1;
            r.empty_since = None;
            let rx = r.tx.subscribe();
            let members = r.members_json();
            r.send(json!({"type": "members", "members": members}));
            (rx, r.snapshot())
        })
    }

    pub fn leave(&self, id: &str, profile: i64) {
        self.with(id, |r| {
            if let Some(e) = r.members.get_mut(&profile) {
                e.1 = e.1.saturating_sub(1);
                if e.1 == 0 {
                    r.members.remove(&profile);
                    if r.voice.remove(&profile) {
                        r.send(json!({"type": "voice", "ids": r.voice}));
                    }
                }
            }
            if r.members.is_empty() {
                r.empty_since = Some(now_ms());
                // Разрыв SSE не означает команду «пауза». Сохраняем часы комнаты,
                // чтобы при обрыве туннеля все вернулись к текущему месту. Пустую
                // комнату janitor удалит через IDLE_MS, как и раньше.
            }
            let members = r.members_json();
            r.send(json!({"type": "members", "members": members}));
        });
    }

    pub fn set_state(&self, id: &str, file_id: i64, playing: bool, position: f64, by: String) -> Option<State> {
        self.with(id, |r| {
            let link_id = r.state.link_id;
            r.state = State {
                revision: r.state.revision.saturating_add(1),
                file_id,
                link_id,
                playing,
                position: position.max(0.0),
                at: now_ms(),
                by,
            };
            r.send(json!({"type": "state", "state": r.state}));
            r.state.clone()
        })
    }

    pub fn say(&self, id: &str, from: Value, body: String) -> Option<()> {
        self.with(id, |r| {
            let m = json!({"type": "chat", "from": from, "body": body, "at": now_ms()});
            r.chat.push(m.clone());
            if r.chat.len() > CHAT_KEEP {
                r.chat.remove(0);
            }
            r.send(m);
        })
    }

    /// Вошёл в голосовой чат или вышел.
    pub fn voice(&self, id: &str, profile: i64, on: bool) -> Option<()> {
        self.with(id, |r| {
            let changed = if on { r.voice.insert(profile) } else { r.voice.remove(&profile) };
            if changed {
                r.send(json!({"type": "voice", "ids": r.voice}));
            }
        })
    }

    /// Сигнал голосового чата (предложение, ответ, адрес) - одному человеку в комнате.
    pub fn signal(&self, id: &str, from: i64, to: i64, data: Value) -> Option<()> {
        self.with(id, |r| {
            if r.voice.contains(&from) && r.members.contains_key(&to) {
                r.send(json!({"type": "signal", "from": from, "to": to, "data": data}));
            }
        })
    }

    pub fn react(&self, id: &str, from: Value, emoji: &str) -> Option<()> {
        self.with(id, |r| r.send(json!({"type": "react", "from": from, "emoji": emoji, "at": now_ms()})))
    }

    /// Закрыть пустые дольше 10 минут.
    pub fn sweep(&self) {
        let now = now_ms();
        self.0.lock().retain(|_, r| r.empty_since.is_none_or(|t| now - t < IDLE_MS));
    }
}

/// Раз в минуту - убрать брошенные комнаты.
pub async fn janitor(app: Arc<crate::App>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        app.rooms.sweep();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_state_leave() {
        let rooms = Rooms::default();
        let id = rooms.create(1, 7, 70, None, 12.0, "Хост".into());
        let (mut rx, snap) = rooms.join(&id, 1, json!({"id": 1, "name": "Хост"})).unwrap();
        assert_eq!(snap["state"]["fileId"], 70);
        assert!(rx.try_recv().unwrap().contains("members"));
        rooms.set_state(&id, 70, true, 30.0, "Хост".into()).unwrap();
        assert!(rx.try_recv().unwrap().contains("\"playing\":true"));
        // Снимок для зашедшего позже - место на сейчас, а не на момент команды.
        rooms.with(&id, |r| r.state.at -= 5_000).unwrap();
        let (_, snap) = rooms.join(&id, 2, json!({"id": 2})).unwrap();
        assert!(snap["state"]["position"].as_f64().unwrap() >= 35.0);
        rooms.leave(&id, 2);
        rooms.leave(&id, 1);
        // Все SSE-соединения оборвались: это не команда паузы. Переподключение
        // получает прежнюю ревизию и актуальное место продолжающегося видео.
        let revision = rooms.with(&id, |r| r.state.revision).unwrap();
        rooms.with(&id, |r| r.state.at -= 3_000).unwrap();
        let (_, reconnected) = rooms.join(&id, 1, json!({"id": 1})).unwrap();
        assert_eq!(reconnected["state"]["playing"], true);
        assert_eq!(reconnected["state"]["revision"], revision);
        assert!(reconnected["state"]["position"].as_f64().unwrap() >= 38.0);
        rooms.leave(&id, 1);
        rooms.sweep();
        assert!(rooms.with(&id, |_| ()).is_some());
        rooms.with(&id, |r| r.empty_since = Some(now_ms() - IDLE_MS - 1)).unwrap();
        rooms.sweep();
        assert!(rooms.with(&id, |_| ()).is_none());
        assert!(rooms.join("нет-такой", 1, json!({})).is_none());
    }

    #[test]
    fn voice_and_signals() {
        let rooms = Rooms::default();
        let id = rooms.create(1, 7, 70, None, 0.0, "А".into());
        let (mut rx, _) = rooms.join(&id, 1, json!({"id": 1})).unwrap();
        rooms.join(&id, 2, json!({"id": 2})).unwrap();
        while rx.try_recv().is_ok() {}
        // Сигнал - только из голосового чата.
        rooms.signal(&id, 1, 2, json!({"type": "offer"})).unwrap();
        assert!(rx.try_recv().is_err());
        rooms.voice(&id, 1, true).unwrap();
        assert!(rx.try_recv().unwrap().contains("\"voice\""));
        rooms.signal(&id, 1, 2, json!({"type": "offer"})).unwrap();
        assert!(rx.try_recv().unwrap().contains("\"signal\""));
        // Ушёл из комнаты - и из голосового чата.
        rooms.leave(&id, 1);
        assert!(rooms.with(&id, |r| r.voice.is_empty()).unwrap());
    }
}
