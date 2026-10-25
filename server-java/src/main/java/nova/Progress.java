package nova;

import com.fasterxml.jackson.databind.*;
import com.sun.net.httpserver.HttpExchange;
import java.util.*;

/** Per-profile playback state, including seasonal overrides and chapter markers. */
final class Progress {
  private static final ObjectMapper JSON = new ObjectMapper();
  private final Database db;

  Progress(Database db) {
    this.db = db;
  }

  private Map<String, Object> file(long id) throws Exception {
    var rows =
        db.query(
            "SELECT f.title_id,COALESCE(f.season,0) season,f.probe FROM files f JOIN titles t ON"
                + " t.id=f.title_id WHERE f.id=? AND f.missing=0 AND t.hidden=0",
            id);
    if (rows.isEmpty()) throw new Main.Failure(404, "Видео не найдено");
    return rows.getFirst();
  }

  private static boolean matches(String title, String... words) {
    String lower = title.toLowerCase(Locale.ROOT);
    var tokens = Set.copyOf(Arrays.asList(lower.split("[^\\p{L}\\p{N}]+")));
    for (String word : words)
      if (word.contains(" ") ? lower.contains(word) : tokens.contains(word)) return true;
    return false;
  }

  Map<String, Object> markers(Map<String, Object> file) throws Exception {
    List<Double> intro = null;
    Double credits = null;
    JsonNode probe = JSON.createObjectNode();
    if (file.get("probe") instanceof String value) {
      try {
        probe = JSON.readTree(value);
      } catch (com.fasterxml.jackson.core.JsonProcessingException ignored) {
      }
    }
    boolean introFound = false;
    for (JsonNode chapter : probe.path("chapters")) {
      double start = chapter.path("start").asDouble(), end = chapter.path("end").asDouble();
      String title = chapter.path("title").asText();
      if (!introFound
          && matches(
              title, "intro", "opening", "op", "заставка", "вступление", "начальные титры")) {
        introFound = true;
        if (end > start && end - start < 300) intro = List.of(start, end);
      }
      if (matches(
          title,
          "credits",
          "ending",
          "ed",
          "end credits",
          "титры",
          "финальные титры",
          "конечные титры")) credits = start;
    }
    if (credits != null
        && probe.hasNonNull("duration")
        && credits <= probe.get("duration").asDouble() * 0.5) credits = null;
    var manual =
        db.query(
            "SELECT intro_start,intro_end,credits FROM markers WHERE title_id=? AND season=?",
            file.get("title_id"),
            file.get("season"));
    if (!manual.isEmpty()) {
      var row = manual.getFirst();
      if (row.get("intro_start") instanceof Number start
          && row.get("intro_end") instanceof Number end
          && end.doubleValue() > start.doubleValue())
        intro = List.of(start.doubleValue(), end.doubleValue());
      if (row.get("credits") instanceof Number value) credits = value.doubleValue();
    }
    var result = new LinkedHashMap<String, Object>();
    result.put("intro", intro);
    result.put("credits", credits);
    result.put("manual", !manual.isEmpty());
    return result;
  }

  boolean handle(HttpExchange exchange, Main.User user, String path, String method)
      throws Exception {
    boolean marker = path.matches("/api/files/[0-9]+/markers");
    if (!marker && !path.matches("/api/progress/[0-9]+")) return false;
    if (!method.equals("PUT") && !(method.equals("DELETE") && !marker))
      throw new Main.Failure(405, "Метод недоступен");
    long id = Long.parseLong(path.split("/")[3]);
    if (method.equals("DELETE")) {
      db.execute(
          "DELETE FROM progress WHERE profile_id=? AND file_id IN (SELECT id FROM files WHERE"
              + " title_id=(SELECT title_id FROM files WHERE id=?))",
          user.profile(),
          id);
      Main.json(exchange, 200, Map.of("ok", true));
      return true;
    }
    var file = file(id);
    var body = Main.body(exchange);
    if (marker) {
      if (user.role().equals("viewer")) throw new Main.Failure(403, "Доступ запрещён");
      Double start = value(body, "introStart"),
          end = value(body, "introEnd"),
          credits = value(body, "credits");
      if (start != null && end != null && (end <= start || end - start > 600))
        throw new Main.Failure(400, "Конец заставки должен быть позже начала (до 10 минут)");
      if (start == null && end == null && credits == null)
        db.execute(
            "DELETE FROM markers WHERE title_id=? AND season=?",
            file.get("title_id"),
            file.get("season"));
      else
        db.execute(
            "INSERT INTO markers(title_id,season,intro_start,intro_end,credits) VALUES(?,?,?,?,?)"
                + " ON CONFLICT(title_id,season) DO UPDATE SET"
                + " intro_start=excluded.intro_start,intro_end=excluded.intro_end,credits=excluded.credits",
            file.get("title_id"),
            file.get("season"),
            start,
            end,
            credits);
      Main.json(exchange, 200, Map.of("ok", true, "markers", markers(file)));
      return true;
    }
    Double position = value(body, "position"), duration = value(body, "duration");
    if (position == null || duration == null || duration <= 0)
      throw new Main.Failure(400, "Неверное время");
    double pos = Math.min(position, duration);
    Double credits = (Double) markers(file).get("credits");
    boolean done = pos >= duration * 0.93 || credits != null && pos >= credits;
    boolean watched =
        db.transaction(
            () -> {
              db.execute(
                  "INSERT INTO progress(profile_id,file_id,position,duration,done,updated_at)"
                      + " VALUES(?,?,?,?,?,?) ON CONFLICT(profile_id,file_id) DO UPDATE SET"
                      + " position=excluded.position,duration=excluded.duration,done=excluded.done,updated_at=excluded.updated_at",
                  user.profile(),
                  id,
                  pos,
                  duration,
                  done,
                  System.currentTimeMillis());
              if (!done) return false;
              long left =
                  ((Number)
                          db.query(
                                  "SELECT COUNT(*) n FROM files f LEFT JOIN progress p ON"
                                      + " p.file_id=f.id AND p.profile_id=? WHERE f.title_id=? AND"
                                      + " f.missing=0 AND COALESCE(p.done,0)=0",
                                  user.profile(),
                                  file.get("title_id"))
                              .getFirst()
                              .get("n"))
                      .longValue();
              if (left > 0) return false;
              db.execute(
                  "INSERT OR IGNORE INTO watched(profile_id,title_id,at) VALUES(?,?,?)",
                  user.profile(),
                  file.get("title_id"),
                  System.currentTimeMillis());
              db.execute(
                  "DELETE FROM my_list WHERE profile_id=? AND title_id=?",
                  user.profile(),
                  file.get("title_id"));
              return true;
            });
    Main.json(exchange, 200, Map.of("ok", true, "done", done, "watched", watched));
    return true;
  }

  private static Double value(JsonNode body, String key) {
    if (!body.hasNonNull(key)) return null;
    JsonNode value = body.get(key);
    if (!value.isNumber() || !Double.isFinite(value.asDouble()) || value.asDouble() < 0)
      throw new Main.Failure(400, "Неверное время");
    return value.asDouble();
  }
}
