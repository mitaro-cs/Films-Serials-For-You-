package nova;

import com.fasterxml.jackson.databind.JsonNode;
import com.sun.net.httpserver.HttpExchange;
import java.util.*;

/** Account mutations preserve the database and authorization rules of the desktop server. */
final class Accounts {
  private final Database db;
  private final Map<Long, ArrayDeque<Long>> passwordFailures = new HashMap<>();

  Accounts(Database db) {
    this.db = db;
  }

  private static String name(String value) {
    String name = value.strip().replaceAll("\\s+", " ");
    if (name.isEmpty() || name.codePointCount(0, name.length()) > 60)
      throw new Main.Failure(400, "Имя — от 1 до 60 символов");
    return name;
  }

  private static void password(String value) {
    if (value.codePointCount(0, value.length()) < 6
        || value.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > 200)
      throw new Main.Failure(400, "Пароль — от 6 символов, не более 200 байт");
  }

  private static void role(Main.User user, String role) {
    if (!role.equals("viewer") && !(role.equals("admin") && user.role().equals("host")))
      throw new Main.Failure(403, "Назначать администраторов может только хост");
  }

  boolean handle(HttpExchange exchange, Main.User user, String path, String method)
      throws Exception {
    if (path.equals("/api/me") && method.equals("PATCH")) {
      JsonNode body = Main.body(exchange);
      String changed = body.hasNonNull("name") ? name(body.get("name").asText()) : null;
      db.transaction(
          () -> {
            if (changed != null) {
              db.execute("UPDATE users SET name=? WHERE id=?", changed, user.id());
              db.execute("UPDATE profiles SET name=? WHERE id=?", changed, user.profile());
            }
            if (body.hasNonNull("avatar"))
              db.execute(
                  "UPDATE profiles SET avatar=? WHERE id=?",
                  Math.floorMod(body.get("avatar").asLong(), 12),
                  user.profile());
            if (body.hasNonNull("autoplay"))
              db.execute(
                  "UPDATE profiles SET autoplay=? WHERE id=?",
                  body.get("autoplay").asBoolean(),
                  user.profile());
            return null;
          });
      Main.json(exchange, 200, Map.of("ok", true));
      return true;
    }
    if (path.equals("/api/me/password") && method.equals("PUT")) {
      JsonNode body = Main.body(exchange);
      String next = body.path("new").asText();
      password(next);
      long now = System.currentTimeMillis();
      synchronized (passwordFailures) {
        var failures = passwordFailures.computeIfAbsent(user.id(), k -> new ArrayDeque<>());
        while (!failures.isEmpty() && failures.getFirst() < now - 600000) failures.removeFirst();
        if (failures.size() >= 5)
          throw new Main.Failure(429, "Слишком много попыток, подождите 10 минут");
      }
      var current = db.query("SELECT pass,login FROM users WHERE id=?", user.id()).getFirst();
      String hash = (String) current.get("pass");
      if (hash != null && !Passwords.verify(body.path("old").asText(), hash)) {
        synchronized (passwordFailures) {
          passwordFailures.get(user.id()).addLast(now);
        }
        throw new Main.Failure(400, "Текущий пароль не подошёл");
      }
      String newHash = Passwords.hash(next);
      db.transaction(
          () -> {
            db.execute("UPDATE users SET pass=? WHERE id=?", newHash, user.id());
            db.execute("DELETE FROM account_tokens WHERE user_id=?", user.id());
            return null;
          });
      synchronized (passwordFailures) {
        passwordFailures.remove(user.id());
      }
      Main.json(exchange, 200, Map.of("ok", true, "login", current.get("login")));
      return true;
    }
    if (!path.equals("/api/admin/users") && !path.matches("/api/admin/users/[0-9]+")) return false;
    if (user.role().equals("viewer")) throw new Main.Failure(403, "Доступ запрещён");
    if (path.equals("/api/admin/users") && method.equals("GET")) {
      var rows =
          db.query(
              "SELECT id,login,name,role,can_download canDownload,disabled,created_at"
                  + " createdAt,seen_at seenAt,pass IS NOT NULL hasPassword FROM users ORDER BY"
                  + " role='host' DESC,role='admin' DESC,name COLLATE NOCASE");
      for (var row : rows)
        for (String key : List.of("canDownload", "disabled", "hasPassword"))
          row.put(key, ((Number) row.get(key)).intValue() != 0);
      Main.json(exchange, 200, rows);
      return true;
    }
    if (path.equals("/api/admin/users") && method.equals("POST")) {
      JsonNode body = Main.body(exchange);
      String login = body.path("login").asText().strip().toLowerCase(Locale.ROOT);
      if (!login.matches("[\\p{L}\\p{N}_.-]{3,32}"))
        throw new Main.Failure(400, "Некорректный логин");
      String name = name(body.path("name").asText()), pw = body.path("password").asText();
      password(pw);
      String role = body.path("role").asText("viewer");
      role(user, role);
      String hash = Passwords.hash(pw);
      db.transaction(
          () -> {
            int changed =
                db.execute(
                    "INSERT OR IGNORE INTO users(login,name,pass,role,created_at)"
                        + " VALUES(?,?,?,?,?)",
                    login,
                    name,
                    hash,
                    role,
                    System.currentTimeMillis());
            if (changed == 0) throw new Main.Failure(409, "Такой логин уже занят");
            db.execute(
                "INSERT INTO profiles(user_id,name,avatar,created_at) SELECT"
                    + " id,name,id%12,created_at FROM users WHERE login=?",
                login);
            return null;
          });
      Main.json(exchange, 200, Map.of("ok", true));
      return true;
    }
    if (!path.matches("/api/admin/users/[0-9]+")) return false;
    long id = Long.parseLong(path.substring(path.lastIndexOf('/') + 1));
    var rows = db.query("SELECT role FROM users WHERE id=?", id);
    if (rows.isEmpty()) throw new Main.Failure(404, "Аккаунт не найден");
    String targetRole = (String) rows.getFirst().get("role");
    if (method.equals("DELETE")) {
      if (targetRole.equals("host")
          || id == user.id()
          || (targetRole.equals("admin") && !user.role().equals("host")))
        throw new Main.Failure(403, "Доступ запрещён");
      db.execute("DELETE FROM users WHERE id=?", id);
    } else if (method.equals("PATCH")) {
      JsonNode body = Main.body(exchange);
      if (targetRole.equals("host")
          && (body.hasNonNull("role")
              || body.path("disabled").asBoolean()
              || body.hasNonNull("password")))
        throw new Main.Failure(403, "Для изменения пароля хоста нужен локальный вход приложения");
      if (!targetRole.equals("viewer") && !user.role().equals("host") && id != user.id())
        throw new Main.Failure(403, "Доступ запрещён");
      if (body.hasNonNull("role")) role(user, body.get("role").asText());
      String changed = body.hasNonNull("name") ? name(body.get("name").asText()) : null;
      String hash = null;
      if (body.hasNonNull("password")) {
        String pw = body.get("password").asText();
        password(pw);
        hash = Passwords.hash(pw);
      }
      String newHash = hash;
      db.transaction(
          () -> {
            if (changed != null) {
              db.execute("UPDATE users SET name=? WHERE id=?", changed, id);
              db.execute("UPDATE profiles SET name=? WHERE user_id=?", changed, id);
            }
            if (body.hasNonNull("role"))
              db.execute("UPDATE users SET role=? WHERE id=?", body.get("role").asText(), id);
            if (body.hasNonNull("canDownload"))
              db.execute(
                  "UPDATE users SET can_download=? WHERE id=?",
                  body.get("canDownload").asBoolean(),
                  id);
            if (body.hasNonNull("disabled")) {
              db.execute(
                  "UPDATE users SET disabled=? WHERE id=?", body.get("disabled").asBoolean(), id);
              if (body.get("disabled").asBoolean())
                db.execute("DELETE FROM sessions WHERE user_id=?", id);
            }
            if (newHash != null) {
              db.execute("UPDATE users SET pass=? WHERE id=?", newHash, id);
              db.execute("DELETE FROM account_tokens WHERE user_id=?", id);
              db.execute("DELETE FROM sessions WHERE user_id=? AND local=0", id);
            }
            return null;
          });
    } else return false;
    Main.json(exchange, 200, Map.of("ok", true));
    return true;
  }
}
