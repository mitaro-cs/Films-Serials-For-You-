package nova;

import com.fasterxml.jackson.databind.*;
import com.sun.net.httpserver.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;

/** Standalone Java backend. Unported routes explicitly return 501; no Rust proxy. */
public final class Main implements AutoCloseable {
  private static final ObjectMapper JSON = new ObjectMapper();
  private final Database db;
  private final Accounts accounts;
  private final Progress progress;
  private final Catalog catalog;
  private final Members members;
  private final EpisodeLocale locale;
  private final Rooms rooms = new Rooms();
  private final HttpServer server;
  private final Path web;
  private final ExecutorService executor = Executors.newVirtualThreadPerTaskExecutor();
  private final Map<String, ArrayDeque<Long>> failures = new HashMap<>();

  record User(long id, long profile, String role, String name, Map<String, Object> person) {}

  static final class Failure extends RuntimeException {
    final int status;

    Failure(int status, String message) {
      super(message);
      this.status = status;
    }

    private static final long serialVersionUID = 1L;
  }

  public Main(Database db, int port, Path web) throws IOException {
    this.db = db;
    this.accounts = new Accounts(db);
    this.members = new Members(db);
    this.progress = new Progress(db);
    this.web = web.toAbsolutePath().normalize();
    locale = new EpisodeLocale(db);
    catalog = new Catalog(db, locale);
    server = HttpServer.create(new InetSocketAddress("127.0.0.1", port), 128);
    server.setExecutor(executor);
    server.createContext("/", this::handle);
  }

  public void start() {
    server.start();
  }

  public int port() {
    return server.getAddress().getPort();
  }

  public static void main(String[] args) throws Exception {
    var options = new HashMap<String, String>();
    for (int i = 0; i < args.length; i += 2) {
      if (i + 1 >= args.length) throw new IllegalArgumentException("Ожидается значение " + args[i]);
      options.put(args[i], args[i + 1]);
    }
    Path database = Path.of(options.getOrDefault("--database", "server-java/data/nova.db"));
    var db = new Database(database);
    var app =
        new Main(
            db,
            Integer.parseInt(options.getOrDefault("--port", "8090")),
            Path.of(options.getOrDefault("--web", "web/build")));
    Runtime.getRuntime()
        .addShutdownHook(
            new Thread(
                () -> {
                  app.close();
                  try {
                    db.close();
                  } catch (Exception ignored) {
                  }
                }));
    app.start();
    System.out.println(
        "Nova Java preview: http://127.0.0.1:" + app.port() + " (перенос API ещё не завершён)");
  }

  private void handle(HttpExchange exchange) throws IOException {
    var headers = exchange.getResponseHeaders();
    headers.set("X-Fsfy", "1");
    headers.set("X-Content-Type-Options", "nosniff");
    headers.set("Cache-Control", "no-store");
    try {
      String path = exchange.getRequestURI().getPath(), method = exchange.getRequestMethod();
      if (!List.of("GET", "HEAD", "POST", "PUT", "PATCH", "DELETE").contains(method))
        throw new Failure(405, "Метод недоступен");
      if (!List.of("GET", "HEAD").contains(method)) csrf(exchange);
      if (path.equals("/api/health")) {
        json(
            exchange,
            200,
            Map.of(
                "ok",
                true,
                "runtime",
                "java",
                "migrationComplete",
                false,
                "version",
                "java-preview.1"));
        return;
      }
      if (path.equals("/api/site")) {
        json(exchange, 200, site());
        return;
      }
      if (path.equals("/api/auth/login") && method.equals("POST")) {
        login(exchange);
        return;
      }
      if (path.equals("/api/auth/logout") && method.equals("POST")) {
        String token = cookie(exchange);
        if (token != null) db.execute("DELETE FROM sessions WHERE hash=?", Passwords.sha256(token));
        exchange
            .getResponseHeaders()
            .add("Set-Cookie", "fsfy_s=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
        json(exchange, 200, Map.of("ok", true));
        return;
      }
      if (!path.startsWith("/api/") && !path.startsWith("/img/")) {
        staticFile(exchange, path);
        return;
      }
      User user = authenticate(exchange);
      if (path.startsWith("/img/") && (method.equals("GET") || method.equals("HEAD"))) {
        String name = path.substring(5);
        if (!name.matches("[0-9a-f]{64}\\.(jpg|png|webp)"))
          throw new Failure(404, "Картинка не найдена");
        Path root = db.directory.resolve("images"), file = root.resolve(name);
        if (!Files.isRegularFile(file) || !file.toRealPath().startsWith(root.toRealPath()))
          throw new Failure(404, "Картинка не найдена");
        exchange.getResponseHeaders().set("Cache-Control", "private, max-age=31536000, immutable");
        Media.send(exchange, file, false);
        return;
      }
      if (accounts.handle(exchange, user, path, method)) return;
      if (progress.handle(exchange, user, path, method)) return;
      if (catalog.handle(exchange, user, path, method)) return;
      if (members.handle(exchange, user, path, method)) return;
      if (path.matches("/api/files/[0-9]+/(raw|download)")
          && (method.equals("GET") || method.equals("HEAD"))) {
        boolean download = path.endsWith("/download");
        if (download
            && (!db.setting("site.downloads", "1").equals("1")
                || ((Number)
                            db.query("SELECT can_download FROM users WHERE id=?", user.id)
                                .getFirst()
                                .get("can_download"))
                        .intValue()
                    == 0)) {
          throw new Failure(403, "Хост не разрешил скачивание");
        }
        var files =
            db.query(
                "SELECT f.path FROM files f JOIN titles t ON t.id=f.title_id WHERE f.id=? AND"
                    + " f.missing=0 AND t.hidden=0",
                Long.parseLong(path.split("/")[3]));
        if (files.isEmpty()) throw new Failure(404, "Видео не найдено");
        Path file = Path.of((String) files.getFirst().get("path"));
        if (!Files.isRegularFile(file)) throw new Failure(404, "Файл недоступен");
        Media.send(exchange, file, download);
        return;
      }
      if (path.equals("/api/me") && method.equals("GET")) {
        var row =
            db.query("SELECT id,login,name,role,can_download FROM users WHERE id=?", user.id)
                .getFirst();
        var card = new LinkedHashMap<>(row);
        card.put("canDownload", ((Number) card.remove("can_download")).intValue() != 0);
        card.put("profile", user.person);
        var profile =
            db.query("SELECT favorites FROM profiles WHERE id=?", user.profile).getFirst();
        json(
            exchange,
            200,
            Map.of(
                "user",
                card,
                "site",
                site(),
                "window",
                false,
                "favorites",
                JSON.readTree(String.valueOf(profile.get("favorites"))),
                "manage",
                !user.role.equals("viewer"),
                "hasPassword",
                db.query("SELECT pass FROM users WHERE id=?", user.id).getFirst().get("pass")
                    != null));
        return;
      }
      if (path.matches("/api/discover/title/tt[0-9]+/episodes") && method.equals("GET")) {
        episodes(exchange, user, path.split("/")[4]);
        return;
      }
      if (path.equals("/api/rooms") && method.equals("POST")) {
        JsonNode body = body(exchange);
        long file = body.path("fileId").asLong(0);
        Long link = body.hasNonNull("linkId") ? body.get("linkId").asLong() : null;
        long title = 0;
        if (file > 0) {
          var rows = db.query("SELECT title_id FROM files WHERE id=? AND missing=0", file);
          if (rows.isEmpty()) throw new Failure(404, "Файл не найден");
          title = ((Number) rows.getFirst().get("title_id")).longValue();
          link = null;
        } else if (link == null || db.query("SELECT id FROM links WHERE id=?", link).isEmpty())
          throw new Failure(404, "Видео не найдено");
        double position = body.path("position").asDouble(0);
        if (!Double.isFinite(position)) throw new Failure(400, "Неверная позиция");
        json(
            exchange,
            200,
            Map.of(
                "id",
                rooms.create(user.profile, title, file, link, Math.max(0, position), user.name)));
        return;
      }
      if (path.startsWith("/api/rooms/")) {
        room(exchange, user, path, method);
        return;
      }
      throw new Failure(
          501, "Этот раздел API ещё не перенесён на Java. Используйте основную версию Nova.");
    } catch (Failure error) {
      json(exchange, error.status, Map.of("error", "request", "message", error.getMessage()));
    } catch (IllegalArgumentException error) {
      json(exchange, 400, Map.of("error", "invalid", "message", "Некорректный запрос"));
    } catch (Exception error) {
      System.err.println("Java API: " + error.getClass().getSimpleName());
      if (exchange.getResponseCode() == -1)
        json(exchange, 500, Map.of("error", "internal", "message", "Ошибка сервера"));
    } finally {
      exchange.close();
    }
  }

  private Map<String, Object> site() throws Exception {
    return Map.of(
        "name",
        db.setting("site.name", "Nova"),
        "metadataLanguage",
        db.setting("meta.language", "ru"),
        "emailRecovery",
        false,
        "registration",
        "closed",
        "downloads",
        db.setting("site.downloads", "1").equals("1"),
        "version",
        "java-preview.1");
  }

  private static String cookie(HttpExchange exchange) {
    String header = exchange.getRequestHeaders().getFirst("Cookie");
    if (header == null) return null;
    for (String part : header.split(";")) {
      var pair = part.strip().split("=", 2);
      if (pair.length == 2 && pair[0].equals("fsfy_s")) return pair[1].replaceAll("^\"|\"$", "");
    }
    return null;
  }

  private User authenticate(HttpExchange exchange) throws Exception {
    String token = cookie(exchange);
    if (token == null || !token.matches("[A-Za-z0-9_-]{16,128}"))
      throw new Failure(401, "Войдите в Nova");
    var rows =
        db.query(
            "SELECT u.id,u.name,u.role,p.id profile,p.avatar,p.photo,p.autoplay FROM sessions s"
                + " JOIN users u ON u.id=s.user_id LEFT JOIN profiles p ON p.user_id=u.id WHERE"
                + " s.hash=? AND s.local=0 AND s.expires_at>? AND u.disabled=0 ORDER BY p.id LIMIT"
                + " 1",
            Passwords.sha256(token),
            System.currentTimeMillis());
    if (rows.isEmpty()) throw new Failure(401, "Войдите в Nova");
    var row = rows.getFirst();
    if (row.get("profile") == null) {
      db.transaction(
          () -> {
            db.execute(
                "INSERT INTO profiles(user_id,name,avatar,created_at) SELECT id,name,id%12,? FROM"
                    + " users WHERE id=? AND NOT EXISTS(SELECT 1 FROM profiles WHERE"
                    + " user_id=users.id)",
                System.currentTimeMillis(), row.get("id"));
            return null;
          });
      return authenticate(exchange);
    }
    long profile = ((Number) row.get("profile")).longValue();
    var person = new LinkedHashMap<String, Object>();
    person.put("id", profile);
    person.put("name", row.get("name"));
    person.put("avatar", row.get("avatar"));
    person.put("photo", Catalog.image(row.get("photo")));
    person.put("autoplay", ((Number) row.get("autoplay")).intValue() != 0);
    person.put("kids", false);
    return new User(
        ((Number) row.get("id")).longValue(),
        profile,
        (String) row.get("role"),
        (String) row.get("name"),
        person);
  }

  private void login(HttpExchange exchange) throws Exception {
    JsonNode body = body(exchange);
    String login = body.path("login").asText("").strip().toLowerCase(Locale.ROOT);
    String password = body.path("password").asText("");
    String key = exchange.getRemoteAddress().getAddress().getHostAddress() + ":" + login;
    long now = System.currentTimeMillis();
    synchronized (failures) {
      var attempts = failures.computeIfAbsent(key, k -> new ArrayDeque<>());
      while (!attempts.isEmpty() && now - attempts.getFirst() > 3600000) attempts.removeFirst();
      if (attempts.size() >= 5) throw new Failure(429, "Слишком много попыток, попробуйте позже");
    }
    var rows = db.query("SELECT id,pass,disabled FROM users WHERE login=?", login);
    boolean valid =
        !rows.isEmpty()
            && ((Number) rows.getFirst().get("disabled")).intValue() == 0
            && Passwords.verify(password, (String) rows.getFirst().get("pass"));
    if (!valid) {
      synchronized (failures) {
        if (failures.size() > 10000) failures.clear();
        failures.computeIfAbsent(key, k -> new ArrayDeque<>()).addLast(now);
      }
      throw new Failure(401, "Неверный логин или пароль");
    }
    synchronized (failures) {
      failures.remove(key);
    }
    String token = Passwords.token();
    db.execute(
        "INSERT INTO sessions(hash,user_id,local,created_at,seen_at,expires_at)"
            + " VALUES(?,?,0,?,?,?)",
        Passwords.sha256(token),
        rows.getFirst().get("id"),
        now,
        now,
        now + 31536000000L);
    exchange
        .getResponseHeaders()
        .add(
            "Set-Cookie", "fsfy_s=" + token + "; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000");
    json(exchange, 200, Map.of("ok", true));
  }

  private void episodes(HttpExchange exchange, User user, String id) throws Exception {
    int season = 1;
    String query = exchange.getRequestURI().getQuery();
    if (query != null)
      for (String pair : query.split("&"))
        if (pair.startsWith("season=")) season = Integer.parseInt(pair.substring(7));
    if (season < 0 || season > 1000) throw new Failure(400, "Неверный сезон");
    var items =
        db.query(
            "SELECT season,episode,name,overview,COALESCE(NULLIF(still,''),still_url)"
                + " still,air_date airDate,runtime,rating,EXISTS(SELECT 1 FROM ext_seen s WHERE"
                + " s.ext_id=e.ext_id AND s.season=e.season AND s.episode=e.episode AND"
                + " s.profile_id=?) seen FROM ext_episodes e WHERE ext_id=? AND season=? ORDER BY"
                + " episode",
            user.profile,
            id,
            season);
    for (var episode : items) {
      String name = (String) episode.get("name"), overview = (String) episode.get("overview");
      locale.queue(name);
      locale.queue(overview);
      episode.put("name", Objects.requireNonNullElse(locale.localized(name), "Серия"));
      episode.put("overview", locale.localized(overview));
      episode.put("seen", ((Number) episode.get("seen")).intValue() != 0);
      String still = (String) episode.get("still");
      if (still != null)
        episode.put(
            "still",
            still.startsWith("https://")
                ? "/api/discover/art?url=" + URLEncoder.encode(still, StandardCharsets.UTF_8)
                : "/img/" + still);
    }
    var seasons =
        db
            .query("SELECT DISTINCT season FROM ext_episodes WHERE ext_id=? ORDER BY season", id)
            .stream()
            .map(row -> row.get("season"))
            .toList();
    json(
        exchange,
        200,
        Map.of("season", season, "seasons", seasons, "items", items, "refreshing", false));
  }

  private void room(HttpExchange exchange, User user, String path, String method) throws Exception {
    String[] parts = path.split("/");
    if (parts.length < 4) throw new Failure(404, "Комната не найдена");
    Rooms.Room room = rooms.get(parts[3]);
    if (room == null) throw new Failure(404, "Комната закрыта");
    if (parts.length == 4 && method.equals("GET")) {
      json(exchange, 200, room.snapshot());
      return;
    }
    String action = parts.length == 5 ? parts[4] : "";
    if (action.equals("events") && method.equals("GET")) {
      exchange.getResponseHeaders().set("Content-Type", "text/event-stream; charset=utf-8");
      exchange.getResponseHeaders().set("Cache-Control", "no-cache");
      exchange.getResponseHeaders().set("X-Accel-Buffering", "no");
      exchange.sendResponseHeaders(200, 0);
      try (var subscription = room.join(user.person);
          var output = exchange.getResponseBody()) {
        while (!Thread.currentThread().isInterrupted()) {
          var event = subscription.next(15000);
          output.write(
              event == null
                  ? ": keepalive\n\n".getBytes(StandardCharsets.UTF_8)
                  : ("data: " + JSON.writeValueAsString(event) + "\n\n")
                      .getBytes(StandardCharsets.UTF_8));
          output.flush();
        }
      } catch (IOException ignored) {
      }
      return;
    }
    if (!method.equals("POST")) throw new Failure(405, "Метод недоступен");
    JsonNode body = body(exchange);
    if (action.equals("state")) {
      long file = room.linkId != null ? room.fileId : body.path("fileId").asLong(room.fileId);
      if (file != room.fileId
          && db.query(
                  "SELECT id FROM files WHERE id=? AND title_id=? AND missing=0",
                  file,
                  room.titleId)
              .isEmpty()) throw new Failure(400, "В комнате доступны только серии этого сериала");
      json(
          exchange,
          200,
          room.command(
              file, body.path("playing").asBoolean(), body.path("position").asDouble(), user.name));
      return;
    }
    if (action.equals("chat")) {
      room.say(user.person, body.path("body").asText().strip());
      json(exchange, 200, Map.of("ok", true));
      return;
    }
    if (action.equals("react")) {
      room.react(user.person, body.path("emoji").asText());
      json(exchange, 200, Map.of("ok", true));
      return;
    }
    throw new Failure(501, "Эта функция комнаты ещё не перенесена на Java");
  }

  private static void csrf(HttpExchange exchange) {
    if (!"1".equals(exchange.getRequestHeaders().getFirst("X-Fsfy")))
      throw new Failure(403, "Запрос отклонён");
    String origin = exchange.getRequestHeaders().getFirst("Origin");
    if (origin != null) {
      URI uri = URI.create(origin);
      if (!Objects.equals(uri.getRawAuthority(), exchange.getRequestHeaders().getFirst("Host")))
        throw new Failure(403, "Запрос с другого сайта отклонён");
    }
  }

  static JsonNode body(HttpExchange exchange) throws IOException {
    byte[] bytes = exchange.getRequestBody().readNBytes(65537);
    if (bytes.length > 65536) throw new Failure(413, "Запрос слишком большой");
    try {
      var value = JSON.readTree(bytes);
      if (value == null || !value.isObject()) throw new Failure(400, "Ожидается объект JSON");
      return value;
    } catch (IOException error) {
      throw new Failure(400, "Некорректный JSON");
    }
  }

  static void json(HttpExchange exchange, int status, Object body) throws IOException {
    byte[] bytes = JSON.writeValueAsBytes(body);
    exchange.getResponseHeaders().set("Content-Type", "application/json; charset=utf-8");
    exchange.sendResponseHeaders(status, bytes.length);
    exchange.getResponseBody().write(bytes);
  }

  private void staticFile(HttpExchange exchange, String path) throws IOException {
    if (!exchange.getRequestMethod().equals("GET") && !exchange.getRequestMethod().equals("HEAD"))
      throw new Failure(405, "Метод недоступен");
    Path file = web.resolve(path.substring(1)).normalize();
    if (!file.startsWith(web)) throw new Failure(404, "Файл не найден");
    if (!Files.isRegularFile(file)) {
      if (path.contains(".")) throw new Failure(404, "Файл не найден");
      file = web.resolve("index.html");
    }
    if (!Files.isRegularFile(file) || !file.toRealPath().startsWith(web.toRealPath()))
      throw new Failure(404, "Файл не найден");
    String name = file.getFileName().toString();
    String type =
        name.endsWith(".js")
            ? "text/javascript"
            : name.endsWith(".css")
                ? "text/css"
                : name.endsWith(".html") ? "text/html" : Files.probeContentType(file);
    exchange
        .getResponseHeaders()
        .set("Content-Type", type == null ? "application/octet-stream" : type);
    if (name.endsWith(".html")) exchange.getResponseHeaders().set("Cache-Control", "no-cache");
    else
      exchange
          .getResponseHeaders()
          .set(
              "Cache-Control",
              path.startsWith("/_app/immutable/")
                  ? "public,max-age=31536000,immutable"
                  : "public,max-age=300");
    if (exchange.getRequestMethod().equals("HEAD")) {
      exchange.getResponseHeaders().set("Content-Length", Long.toString(Files.size(file)));
      exchange.sendResponseHeaders(200, -1);
      return;
    }
    exchange.sendResponseHeaders(200, Files.size(file));
    try (var input = Files.newInputStream(file)) {
      input.transferTo(exchange.getResponseBody());
    }
  }

  @Override
  public void close() {
    server.stop(0);
    locale.close();
    executor.shutdownNow();
  }
}
