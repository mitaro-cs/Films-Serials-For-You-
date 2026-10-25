package nova;

import com.sun.net.httpserver.HttpExchange;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Member list uses one query, including external watches and friendship state. */
final class Members {
  private final Database db;

  Members(Database db) {
    this.db = db;
  }

  private static String norm(String value) {
    return value
        .toLowerCase(Locale.ROOT)
        .replace('ё', 'е')
        .replaceAll("[^\\p{L}\\p{N}]+", " ")
        .strip();
  }

  boolean handle(HttpExchange exchange, Main.User user, String path, String method)
      throws Exception {
    boolean friendship =
        path.matches("/api/friends/[0-9]+") && List.of("POST", "DELETE").contains(method);
    if (!friendship && !(path.equals("/api/members") && method.equals("GET"))) return false;
    if (((Number)
                db.query("SELECT kids FROM profiles WHERE id=?", user.profile())
                    .getFirst()
                    .get("kids"))
            .intValue()
        != 0) throw new Main.Failure(403, "В детском профиле общение выключено");
    if (friendship) {
      long other = Long.parseLong(path.substring(path.lastIndexOf('/') + 1));
      String relation =
          db.transaction(
              () -> {
                if (method.equals("DELETE")) {
                  db.execute(
                      "DELETE FROM friends WHERE (a=? AND b=?) OR (a=? AND b=?)",
                      user.profile(),
                      other,
                      other,
                      user.profile());
                  return "none";
                }
                if (other == user.profile()) throw new Main.Failure(400, "Это вы");
                if (db.query(
                        "SELECT p.id FROM profiles p JOIN users u ON u.id=p.user_id WHERE p.id=?"
                            + " AND p.kids=0 AND u.disabled=0",
                        other)
                    .isEmpty()) throw new Main.Failure(404, "Участник не найден");
                var current =
                    db.query(
                        "SELECT a,status FROM friends WHERE (a=? AND b=?) OR (a=? AND b=?)",
                        user.profile(),
                        other,
                        other,
                        user.profile());
                if (current.isEmpty()) {
                  db.execute(
                      "INSERT INTO friends(a,b,status,created_at) VALUES(?,?,'pending',?)",
                      user.profile(),
                      other,
                      System.currentTimeMillis());
                  return "outgoing";
                }
                var row = current.getFirst();
                if (row.get("status").equals("accepted")) return "friend";
                if (((Number) row.get("a")).longValue() == user.profile()) return "outgoing";
                db.execute(
                    "UPDATE friends SET status='accepted' WHERE a=? AND b=?",
                    other,
                    user.profile());
                return "friend";
              });
      Main.json(exchange, 200, Map.of("relation", relation));
      return true;
    }
    String query = "";
    for (String part : Objects.toString(exchange.getRequestURI().getRawQuery(), "").split("&")) {
      if (part.startsWith("q="))
        query = norm(URLDecoder.decode(part.substring(2), StandardCharsets.UTF_8));
    }
    var rows =
        db.query(
            """
            SELECT p.id,p.name,p.avatar,p.photo,
              (SELECT COUNT(*) FROM (
                SELECT COALESCE(NULLIF(t.imdb_id,''),'local:'||t.id) work
                FROM watched w JOIN titles t ON t.id=w.title_id
                WHERE w.profile_id=p.id AND t.kind='movie'
                UNION SELECT m.ext_id FROM ext_marks m JOIN ext_titles e ON e.id=m.ext_id
                WHERE m.profile_id=p.id AND m.status='watched' AND e.kind='movie'
              )) watched,
              (SELECT COUNT(*) FROM reviews r WHERE r.profile_id=p.id) reviews,
              COALESCE((SELECT CASE WHEN f.status='accepted' THEN 'friend' WHEN f.a=? THEN 'outgoing' ELSE 'incoming' END
                FROM friends f WHERE (f.a=? AND f.b=p.id) OR (f.b=? AND f.a=p.id) LIMIT 1),'none') relation
            FROM profiles p JOIN users u ON u.id=p.user_id
            WHERE p.kids=0 AND u.disabled=0 AND p.id!=? ORDER BY p.name COLLATE NOCASE LIMIT 500
            """,
            user.profile(),
            user.profile(),
            user.profile(),
            user.profile());
    var result = new ArrayList<Map<String, Object>>();
    for (var row : rows) {
      if (!norm((String) row.get("name")).contains(query)) continue;
      var item = new LinkedHashMap<>(row);
      item.put("photo", Catalog.image(row.get("photo")));
      result.add(item);
    }
    var order = List.of("incoming", "friend", "outgoing", "none");
    result.sort(Comparator.comparingInt(row -> order.indexOf(row.get("relation"))));
    Main.json(exchange, 200, result);
    return true;
  }
}
