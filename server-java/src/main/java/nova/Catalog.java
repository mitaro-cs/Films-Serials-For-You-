package nova;

import com.fasterxml.jackson.databind.*;
import com.sun.net.httpserver.HttpExchange;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.*;

/** Local catalogue contracts; metadata discovery remains a separate provider. */
final class Catalog {
  private static final ObjectMapper JSON = new ObjectMapper();
  private final Database db;
  private final EpisodeLocale locale;

  Catalog(Database db, EpisodeLocale locale) {
    this.db = db;
    this.locale = locale;
  }

  static String image(Object name) {
    return name == null ? null : "/img/" + name;
  }

  static List<String> strings(Object raw) {
    var out = new ArrayList<String>();
    if (raw instanceof String value)
      try {
        for (JsonNode node : JSON.readTree(value)) if (node.isTextual()) out.add(node.asText());
      } catch (Exception ignored) {
      }
    return out;
  }

  static String genre(String value) {
    return switch (value) {
      case "Action" -> "Боевик";
      case "Adventure" -> "Приключения";
      case "Animation" -> "Анимация";
      case "Biography" -> "Биография";
      case "Comedy" -> "Комедия";
      case "Crime" -> "Криминал";
      case "Documentary" -> "Документальный";
      case "Drama" -> "Драма";
      case "Family" -> "Семейный";
      case "Fantasy" -> "Фэнтези";
      case "History" -> "История";
      case "Horror" -> "Ужасы";
      case "Music" -> "Музыка";
      case "Musical" -> "Мюзикл";
      case "Mystery" -> "Детектив";
      case "Romance" -> "Мелодрама";
      case "Sci-Fi" -> "Фантастика";
      case "Sport" -> "Спорт";
      case "Thriller" -> "Триллер";
      case "War" -> "Военный";
      case "Western" -> "Вестерн";
      default -> value;
    };
  }

  static Map<String, String> query(HttpExchange exchange) {
    var out = new HashMap<String, String>();
    String raw = exchange.getRequestURI().getRawQuery();
    if (raw != null)
      for (String pair : raw.split("&")) {
        String[] parts = pair.split("=", 2);
        out.put(
            URLDecoder.decode(parts[0], StandardCharsets.UTF_8),
            parts.length == 2 ? URLDecoder.decode(parts[1], StandardCharsets.UTF_8) : "");
      }
    return out;
  }

  private static long number(Object value) {
    return value instanceof Number n ? n.longValue() : 0;
  }

  // Only typed database IDs, never request text, are used in these bulk predicates.
  private static String ids(Collection<Long> values) {
    return values.stream().map(String::valueOf).collect(java.util.stream.Collectors.joining(","));
  }

  private static String norm(Object value) {
    return Objects.toString(value, "")
        .toLowerCase(Locale.ROOT)
        .replace('ё', 'е')
        .replaceAll("[^\\p{L}\\p{N}]+", " ")
        .strip();
  }

  private static final String COLUMNS =
      """
      SELECT t.id,t.kind,t.title,t.original_title originalTitle,t.year,
      COALESCE(t.poster_sm,t.poster) poster,t.poster posterLarge,
      COALESCE(t.backdrop_md,t.backdrop,(SELECT still FROM files WHERE title_id=t.id AND still IS NOT NULL ORDER BY season,episode LIMIT 1)) backdrop,
      t.logo,t.rating,t.genres,t.overview,
      COALESCE(t.runtime,CASE WHEN t.kind='movie' THEN (SELECT CAST(ROUND(SUM(duration)/60.0) AS INTEGER) FROM files WHERE title_id=t.id AND missing=0 AND duration>0) END) runtime,
      t.age,t.added_at addedAt,
      (SELECT COUNT(*) FROM files WHERE title_id=t.id AND missing=0) files,
      (SELECT COUNT(DISTINCT season) FROM files WHERE title_id=t.id AND missing=0) seasons,
      json_extract(t.countries,'$[0]') country,
      (SELECT json_extract(value,'$.name') FROM json_each(t.people) WHERE json_extract(value,'$.job')='director' LIMIT 1) director,t.accent,
      COALESCE(t.backdrop_sm,(SELECT still FROM files WHERE title_id=t.id AND still IS NOT NULL ORDER BY season,episode LIMIT 1),t.backdrop) backdropSmall
      FROM titles t WHERE t.hidden=0 AND EXISTS(SELECT 1 FROM files WHERE title_id=t.id AND missing=0)
      """;

  private List<Map<String, Object>> cards(
      long profile, String filter, String order, Object... params) throws Exception {
    var rows =
        db.query(COLUMNS + " AND (" + filter + ") ORDER BY " + order + " LIMIT 5000", params);
    var taste = new Taste(profile);
    for (var row : rows) {
      for (String key : List.of("poster", "posterLarge", "backdrop", "logo", "backdropSmall"))
        row.put(key, image(row.get(key)));
      row.put("genres", strings(row.get("genres")).stream().map(Catalog::genre).toList());
      taste.apply(row);
      row.put("resume", null);
    }
    return rows;
  }

  private final class Taste {
    final Map<Long, Integer> ratings = new HashMap<>();
    final Set<Long> list = new HashSet<>(), manual = new HashSet<>();
    final Map<Long, Long> seen = new HashMap<>(),
        dropped = new HashMap<>(),
        last = new LinkedHashMap<>();
    final Map<String, Double> weights = new HashMap<>();

    Taste(long profile) throws Exception {
      var titleWeights = new HashMap<Long, Double>();
      for (var r :
          db.query(
              "SELECT f.title_id id,MAX(p.done) done,MAX(p.updated_at) at FROM progress p JOIN"
                  + " files f ON f.id=p.file_id WHERE profile_id=? GROUP BY f.title_id ORDER BY at"
                  + " DESC",
              profile)) {
        long id = number(r.get("id"));
        last.put(id, number(r.get("at")));
        titleWeights.put(id, number(r.get("done")) != 0 ? 2.0 : 1.0);
      }
      for (var r : db.query("SELECT title_id,at FROM watched WHERE profile_id=?", profile)) {
        long id = number(r.get("title_id"));
        seen.put(id, number(r.get("at")));
        titleWeights.put(id, 2.0);
      }
      for (var r : db.query("SELECT title_id,at FROM dropped WHERE profile_id=?", profile))
        dropped.put(number(r.get("title_id")), number(r.get("at")));
      for (var r : db.query("SELECT title_id FROM watching WHERE profile_id=?", profile))
        manual.add(number(r.get("title_id")));
      for (var r : db.query("SELECT title_id FROM my_list WHERE profile_id=?", profile)) {
        long id = number(r.get("title_id"));
        list.add(id);
        titleWeights.merge(id, 1.0, Double::sum);
      }
      for (var r : db.query("SELECT title_id,value FROM ratings WHERE profile_id=?", profile)) {
        long id = number(r.get("title_id"));
        int value = (int) number(r.get("value"));
        ratings.put(id, value);
        titleWeights.put(id, value == 2 ? 3.0 : value == 1 ? 2.0 : -3.0);
      }
      // One bulk query rather than one database round trip per history item.
      if (!titleWeights.isEmpty())
        for (var r :
            db.query(
                "SELECT id,genres FROM titles WHERE id IN (" + ids(titleWeights.keySet()) + ")")) {
          Double weight = titleWeights.get(number(r.get("id")));
          if (weight != null)
            for (String genre : strings(r.get("genres")))
              weights.merge(genre(genre), weight, Double::sum);
        }
    }

    boolean dropped(long id) {
      return dropped.containsKey(id) && last.getOrDefault(id, 0L) <= dropped.get(id);
    }

    void apply(Map<String, Object> card) {
      long id = number(card.get("id"));
      card.put("inList", list.contains(id));
      card.put("myRating", ratings.get(id));
      card.put("watchedAt", seen.get(id));
      card.put(
          "status",
          seen.containsKey(id)
              ? "watched"
              : manual.contains(id) || last.containsKey(id) && !dropped(id)
                  ? "watching"
                  : list.contains(id) ? "want" : dropped(id) ? "dropped" : null);
      Integer match = null;
      if (weights.values().stream().filter(v -> v > 0).mapToDouble(Double::doubleValue).sum() >= 2
          && card.get("genres") instanceof List<?> genres
          && !genres.isEmpty()) {
        double max = weights.values().stream().mapToDouble(Double::doubleValue).max().orElse(0);
        if (max > 0) {
          double mean =
              genres.stream()
                  .mapToDouble(g -> Math.clamp(weights.getOrDefault(g, 0.0) / max, -1, 1))
                  .boxed()
                  .sorted(Comparator.reverseOrder())
                  .limit(2)
                  .mapToDouble(Double::doubleValue)
                  .average()
                  .orElse(0);
          int pct = (int) Math.round(55 + 44 * Math.clamp(mean, 0, 1));
          if (pct >= 70) match = pct;
        }
      }
      card.put("matchPct", match);
    }
  }

  private Map<Long, List<Map<String, Object>>> files(long profile, Collection<Long> titles)
      throws Exception {
    var grouped = new HashMap<Long, List<Map<String, Object>>>();
    var rows =
        db.query(
            "SELECT f.title_id,f.id,f.season,f.episode,f.episode_end"
                + " episodeEnd,f.name,f.overview,f.still,f.air_date"
                + " airDate,f.duration,f.size,f.path,f.added_at"
                + " addedAt,f.rating,p.position,p.duration progressDuration,p.done,p.updated_at"
                + " FROM files f LEFT JOIN progress p ON p.file_id=f.id AND p.profile_id=? WHERE"
                + " f.missing=0 AND f.title_id IN ("
                + ids(titles)
                + ") ORDER BY f.season,f.episode,f.path",
            profile);
    for (var row : rows) {
      long title = number(row.remove("title_id"));
      row.put("fileName", Path.of((String) row.remove("path")).getFileName().toString());
      row.put("still", image(row.get("still")));
      row.put("name", locale.localized((String) row.get("name")));
      row.put("overview", locale.localized((String) row.get("overview")));
      Object position = row.remove("position"),
          duration = row.remove("progressDuration"),
          done = row.remove("done");
      row.put(
          "progress",
          position == null
              ? null
              : Map.of("position", position, "duration", duration, "done", number(done) != 0));
      grouped.computeIfAbsent(title, k -> new ArrayList<>()).add(row);
    }
    return grouped;
  }

  private static Map<String, Object> next(List<Map<String, Object>> files) {
    if (files == null || files.isEmpty()) return null;
    int pick = 0;
    long last = -1;
    for (int i = 0; i < files.size(); i++)
      if (files.get(i).get("updated_at") instanceof Number n && n.longValue() > last) {
        last = n.longValue();
        pick = i;
      }
    if (files.get(pick).get("progress") instanceof Map<?, ?> p
        && Boolean.TRUE.equals(p.get("done"))) pick = Math.min(pick + 1, files.size() - 1);
    var f = files.get(pick);
    Map<?, ?> p =
        f.get("progress") instanceof Map<?, ?> m && !Boolean.TRUE.equals(m.get("done")) ? m : null;
    String label =
        f.get("season") != null && f.get("episode") != null
            ? f.get("season")
                + " сезон, "
                + f.get("episode")
                + " серия"
                + (f.get("name") == null ? "" : " · " + f.get("name"))
            : "";
    return Map.of(
        "fileId",
        f.get("id"),
        "label",
        label,
        "position",
        p == null ? 0 : p.get("position"),
        "duration",
        p == null ? Objects.requireNonNullElse(f.get("duration"), 0) : p.get("duration"));
  }

  private void resume(long profile, List<Map<String, Object>> cards) throws Exception {
    if (cards.stream().noneMatch(c -> "watching".equals(c.get("status")))) return;
    var files =
        files(
            profile,
            cards.stream()
                .filter(c -> "watching".equals(c.get("status")))
                .map(c -> number(c.get("id")))
                .toList());
    for (var card : cards)
      if ("watching".equals(card.get("status")))
        card.put("resume", next(files.get(number(card.get("id")))));
  }

  boolean handle(HttpExchange exchange, Main.User user, String path, String method)
      throws Exception {
    if (method.equals("GET")
        && Set.of("/api/titles", "/api/library", "/api/list", "/api/genres").contains(path)) {
      var q = query(exchange);
      long me = user.profile();
      if (path.equals("/api/genres")) {
        Main.json(
            exchange,
            200,
            db.query(
                "SELECT g.value name,COUNT(*) count FROM titles t,json_each(t.genres) g WHERE"
                    + " t.hidden=0 AND EXISTS(SELECT 1 FROM files WHERE title_id=t.id AND"
                    + " missing=0) GROUP BY g.value ORDER BY count DESC,g.value"));
        return true;
      }
      String kind =
          path.equals("/api/library")
              ? switch (q.getOrDefault("tab", "")) {
                case "movies" -> "movie";
                case "series" -> "series";
                default -> null;
              }
              : q.get("kind");
      if (!"movie".equals(kind) && !"series".equals(kind)) kind = null;
      String filter = "(? IS NULL OR t.kind=?)";
      var params = new ArrayList<Object>(Arrays.asList(kind, kind));
      String order = "t.added_at DESC,t.id DESC";
      if (path.equals("/api/titles")) {
        order =
            switch (q.getOrDefault("sort", "")) {
              case "rating" -> "t.rating IS NULL,t.rating DESC";
              case "year" -> "t.year IS NULL,t.year DESC";
              case "title" -> "t.title COLLATE NOCASE";
              default -> order;
            };
        if (!q.getOrDefault("genre", "").isEmpty()) {
          filter += " AND EXISTS(SELECT 1 FROM json_each(t.genres) WHERE value=?)";
          params.add(q.get("genre"));
        }
      }
      if (path.equals("/api/list")) {
        filter = "t.id IN(SELECT title_id FROM my_list WHERE profile_id=?)";
        params.clear();
        params.add(me);
        order = "(SELECT added_at FROM my_list WHERE title_id=t.id AND profile_id=" + me + ") DESC";
      }
      var items = cards(me, filter, order, params.toArray());
      if (path.equals("/api/titles") && !q.getOrDefault("q", "").isBlank()) {
        var words = norm(q.get("q")).strip().split("\\s+");
        items.removeIf(
            c -> {
              String hay =
                  norm(c.get("title"))
                      + " "
                      + norm(c.get("originalTitle"))
                      + " "
                      + norm(c.get("year"));
              return Arrays.stream(words).anyMatch(w -> !hay.contains(w));
            });
      }
      if (path.equals("/api/library")) {
        if ("new".equals(q.get("tab")) && items.size() > 60)
          items.subList(60, items.size()).clear();
        var counts = new LinkedHashMap<String, Object>();
        counts.put("all", items.size());
        for (String status : List.of("want", "watching", "watched"))
          counts.put(status, items.stream().filter(c -> status.equals(c.get("status"))).count());
        var taste = new Taste(me);
        var ids = new HashSet<Long>();
        for (var c : items) if ("watching".equals(c.get("status"))) ids.add(number(c.get("id")));
        var recent =
            taste.last.keySet().stream()
                .filter(id -> !taste.dropped(id) && ids.contains(id))
                .toList();
        String status = q.get("status");
        if (status != null && List.of("want", "watching", "watched").contains(status)) {
          items.removeIf(c -> !status.equals(c.get("status")));
          if (status.equals("watched"))
            items.sort(
                Comparator.comparingLong((Map<String, Object> c) -> number(c.get("watchedAt")))
                    .reversed());
          if (status.equals("watching"))
            items.sort(
                Comparator.comparingInt(
                    c -> {
                      int i = recent.indexOf(number(c.get("id")));
                      return i < 0 ? Integer.MAX_VALUE : i;
                    }));
        }
        resume(me, items);
        Main.json(exchange, 200, Map.of("items", items, "counts", counts, "recent", recent));
        return true;
      }
      if (!path.equals("/api/list")) resume(me, items);
      Main.json(exchange, 200, items);
      return true;
    }
    if (!path.matches("/api/titles/[0-9]+/(rating|watched|status)")
        && !path.matches("/api/list/[0-9]+")) return false;
    if (!method.equals("PUT")) throw new Main.Failure(405, "Метод недоступен");
    long id = Long.parseLong(path.split("/")[3]),
        me = user.profile(),
        now = System.currentTimeMillis();
    var b = Main.body(exchange);
    String action = path.startsWith("/api/list/") ? "list" : path.split("/")[4];
    if (action.equals("list") || action.equals("rating")) {
      if (action.equals("rating")) {
        if (!b.path("value").isIntegralNumber()
            || !b.path("value").canConvertToInt()
            || b.path("value").asInt() < -1
            || b.path("value").asInt() > 2) throw new Main.Failure(400, "Оценка: -1, 0, 1 или 2");
        int value = b.get("value").asInt();
        if (value == 0) db.execute("DELETE FROM ratings WHERE profile_id=? AND title_id=?", me, id);
        else
          db.execute(
              "INSERT INTO ratings(profile_id,title_id,value,updated_at) SELECT ?,id,?,? FROM"
                  + " titles WHERE id=? ON CONFLICT(profile_id,title_id) DO UPDATE SET"
                  + " value=excluded.value,updated_at=excluded.updated_at",
              me,
              value,
              now,
              id);
      } else {
        if (!b.path("value").isBoolean()) throw new Main.Failure(400, "Неверное значение");
        if (b.get("value").asBoolean())
          db.execute(
              "INSERT OR IGNORE INTO my_list(profile_id,title_id,added_at) SELECT ?,id,? FROM"
                  + " titles WHERE id=?",
              me,
              now,
              id);
        else db.execute("DELETE FROM my_list WHERE profile_id=? AND title_id=?", me, id);
      }
      Main.json(exchange, 200, Map.of("ok", true));
      return true;
    }
    var cards = cards(me, "t.id=?", "t.id", id);
    if (cards.isEmpty()) throw new Main.Failure(404, "Фильм не найден");
    String status = b.hasNonNull("status") ? b.get("status").asText() : null;
    if (action.equals("status")
        && status != null
        && !List.of("want", "watching", "watched", "dropped").contains(status))
      throw new Main.Failure(400, "Неверный статус");
    if (action.equals("watched") && !b.path("value").isBoolean())
      throw new Main.Failure(400, "Неверное значение");
    db.transaction(
        () -> {
          if (action.equals("watched")) {
            if (b.get("value").asBoolean()) {
              db.execute(
                  "INSERT OR IGNORE INTO watched(profile_id,title_id,at) VALUES(?,?,?)",
                  me,
                  id,
                  now);
              drop("my_list", me, id);
            } else {
              drop("watched", me, id);
              db.execute(
                  "DELETE FROM progress WHERE profile_id=? AND file_id IN(SELECT id FROM files"
                      + " WHERE title_id=?)",
                  me,
                  id);
            }
          } else {
            String[] remove =
                status == null
                    ? new String[] {"my_list", "watching", "watched"}
                    : switch (status) {
                      case "want" -> new String[] {"watched", "watching"};
                      case "watching" -> new String[] {"watched", "dropped", "my_list"};
                      case "watched" -> new String[] {"watching", "dropped", "my_list"};
                      default -> new String[] {"watching", "my_list"};
                    };
            for (String table : remove) drop(table, me, id);
            if (status != null) {
              String table = status.equals("want") ? "my_list" : status;
              String col = status.equals("want") ? "added_at" : "at";
              db.execute(
                  "INSERT INTO "
                      + table
                      + "(profile_id,title_id,"
                      + col
                      + ") VALUES(?,?,?) ON CONFLICT DO UPDATE SET "
                      + col
                      + "=excluded."
                      + col,
                  me,
                  id,
                  now);
            }
          }
          return null;
        });
    if (action.equals("status")) {
      var c = cards(me, "t.id=?", "t.id", id);
      resume(me, c);
      Main.json(exchange, 200, c.getFirst());
    } else Main.json(exchange, 200, Map.of("ok", true));
    return true;
  }

  private void drop(String table, long profile, long title) throws Exception {
    db.execute("DELETE FROM " + table + " WHERE profile_id=? AND title_id=?", profile, title);
  }
}
