package nova;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.net.*;
import java.net.http.*;
import java.util.*;

final class AccountProgressTest {
  private static final ObjectMapper JSON = new ObjectMapper();

  private AccountProgressTest() {}

  static HttpResponse<String> request(
      HttpClient client, String base, String method, String path, Object body) throws Exception {
    return client.send(
        HttpRequest.newBuilder(URI.create(base + path))
            .header("X-Fsfy", "1")
            .header("Content-Type", "application/json")
            .method(
                method,
                body == null
                    ? HttpRequest.BodyPublishers.noBody()
                    : HttpRequest.BodyPublishers.ofString(JSON.writeValueAsString(body)))
            .build(),
        HttpResponse.BodyHandlers.ofString());
  }

  static void run(Database db, HttpClient viewer, String base, String hash) throws Exception {
    assert request(viewer, base, "GET", "/api/admin/users", null).statusCode() == 403;
    assert request(
                viewer,
                base,
                "PATCH",
                "/api/me",
                Map.of("name", "  Иван   Петров  ", "autoplay", false))
            .statusCode()
        == 200;
    assert db.query("SELECT name FROM users WHERE id=1")
        .getFirst()
        .get("name")
        .equals("Иван Петров");
    assert db.query("SELECT name FROM profiles WHERE id=1")
        .getFirst()
        .get("name")
        .equals("Иван Петров");
    assert request(viewer, base, "PATCH", "/api/me", Map.of("name", " ", "avatar", 5)).statusCode()
        == 400;
    assert request(
                viewer,
                base,
                "PUT",
                "/api/me/password",
                Map.of("old", "wrong", "new", "NewPassword123"))
            .statusCode()
        == 400;
    assert Passwords.verify(
        "MigrationPassword123",
        (String) db.query("SELECT pass FROM users WHERE id=1").getFirst().get("pass"));
    var bad =
        request(viewer, base, "PUT", "/api/progress/1", Map.of("position", -1, "duration", 100));
    assert bad.statusCode() == 400 : bad.body();
    var halfway =
        request(viewer, base, "PUT", "/api/progress/1", Map.of("position", 50, "duration", 100));
    assert halfway.statusCode() == 200 : halfway.body();
    assert !JSON.readTree(halfway.body()).path("done").asBoolean();
    assert request(viewer, base, "PUT", "/api/files/1/markers", Map.of("credits", 80)).statusCode()
        == 403;
    long now = System.currentTimeMillis();
    db.execute(
        "INSERT INTO users(id,login,name,pass,role,created_at) VALUES(2,'host','Хост',?,'host',?)",
        hash,
        now);
    db.execute("INSERT INTO profiles(id,user_id,name,created_at) VALUES(2,2,'Хост',?)", now);
    try (var host =
        HttpClient.newBuilder()
            .cookieHandler(new CookieManager(null, CookiePolicy.ACCEPT_ALL))
            .build()) {
      assert request(
                  host,
                  base,
                  "POST",
                  "/api/auth/login",
                  Map.of("login", "host", "password", "MigrationPassword123"))
              .statusCode()
          == 200;
      assert request(
                  host,
                  base,
                  "PUT",
                  "/api/files/1/markers",
                  Map.of("introStart", 20, "introEnd", 10))
              .statusCode()
          == 400;
      assert request(host, base, "PUT", "/api/files/1/markers", Map.of("credits", 80)).statusCode()
          == 200;
      var finished =
          request(viewer, base, "PUT", "/api/progress/1", Map.of("position", 81, "duration", 100));
      assert finished.statusCode() == 200 : finished.body();
      assert JSON.readTree(finished.body()).path("done").asBoolean();
      assert JSON.readTree(finished.body()).path("watched").asBoolean();
      assert db.query("SELECT * FROM progress WHERE profile_id=2").isEmpty();
      assert request(viewer, base, "DELETE", "/api/progress/1", null).statusCode() == 200;
      assert db.query("SELECT * FROM progress WHERE profile_id=1").isEmpty();
      assert request(host, base, "PATCH", "/api/admin/users/2", Map.of("disabled", true))
              .statusCode()
          == 403;
      assert request(
                  host,
                  base,
                  "POST",
                  "/api/admin/users",
                  Map.of("login", "newuser", "name", "Новый", "password", "Password123"))
              .statusCode()
          == 200;
      assert request(
                  host,
                  base,
                  "POST",
                  "/api/admin/users",
                  Map.of("login", "newuser", "name", "Другой", "password", "Password123"))
              .statusCode()
          == 409;
      assert db.query(
                  "SELECT p.id FROM profiles p JOIN users u ON u.id=p.user_id WHERE"
                      + " u.login='newuser'")
              .size()
          == 1;
      assert request(
                  host, base, "PATCH", "/api/admin/users/1", Map.of("password", "ResetPassword123"))
              .statusCode()
          == 200;
      assert request(viewer, base, "GET", "/api/me", null).statusCode() == 401;
      assert request(
                  viewer,
                  base,
                  "POST",
                  "/api/auth/login",
                  Map.of("login", "viewer", "password", "MigrationPassword123"))
              .statusCode()
          == 401;
      assert request(
                  viewer,
                  base,
                  "POST",
                  "/api/auth/login",
                  Map.of("login", "viewer", "password", "ResetPassword123"))
              .statusCode()
          == 200;
      assert request(
                  viewer,
                  base,
                  "PUT",
                  "/api/me/password",
                  Map.of("old", "ResetPassword123", "new", "OwnPassword123"))
              .statusCode()
          == 200;
      assert Passwords.verify(
          "OwnPassword123",
          (String) db.query("SELECT pass FROM users WHERE id=1").getFirst().get("pass"));
      assert request(host, base, "PATCH", "/api/admin/users/1", Map.of("disabled", true))
              .statusCode()
          == 200;
      assert request(viewer, base, "GET", "/api/me", null).statusCode() == 401;
    }
    System.out.println(
        "PASS: account authorization, profile changes, reset/session revocation, progress and"
            + " seasonal markers.");
  }
}
