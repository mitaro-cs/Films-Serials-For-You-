package nova;

import com.fasterxml.jackson.databind.*;
import java.net.http.HttpClient;
import java.util.Map;

final class CatalogTest {
  private static final ObjectMapper JSON = new ObjectMapper();

  private CatalogTest() {}

  static JsonNode call(HttpClient client, String base, String method, String path, Object body)
      throws Exception {
    var response = AccountProgressTest.request(client, base, method, path, body);
    assert response.statusCode() == 200 : response.statusCode() + " " + response.body();
    return JSON.readTree(response.body());
  }

  static void run(Database db, HttpClient client, String base) throws Exception {
    db.execute("UPDATE titles SET title='Ёлки',genres='[\"Комедия\"]' WHERE id=1");
    var library = call(client, base, "GET", "/api/library", null);
    assert library.path("items").size() == 1 : library;
    assert library.path("counts").path("all").asInt() == 1;
    assert call(client, base, "GET", "/api/titles?q=%D0%B5%D0%BB%D0%BA%D0%B8", null).size() == 1;
    assert call(client, base, "GET", "/api/titles?q=absent", null).isEmpty();
    assert call(client, base, "GET", "/api/library?tab=series", null).path("items").isEmpty();
    assert call(client, base, "GET", "/api/genres", null)
        .get(0)
        .path("name")
        .asText()
        .equals("Комедия");
    call(client, base, "PUT", "/api/list/1", Map.of("value", true));
    assert call(client, base, "GET", "/api/list", null).get(0).path("inList").asBoolean();
    assert call(client, base, "PUT", "/api/titles/1/status", Map.of("status", "watching"))
        .path("status")
        .asText()
        .equals("watching");
    call(client, base, "PUT", "/api/progress/1", Map.of("position", 35, "duration", 100));
    library = call(client, base, "GET", "/api/library?status=watching", null);
    assert library.path("items").get(0).path("resume").path("position").asInt() == 35 : library;
    assert library.path("recent").get(0).asInt() == 1;
    call(client, base, "PUT", "/api/titles/1/rating", Map.of("value", 2));
    assert call(client, base, "GET", "/api/titles", null).get(0).path("matchPct").asInt() == 99;
    assert AccountProgressTest.request(
                client, base, "PUT", "/api/titles/1/rating", Map.of("value", 5))
            .statusCode()
        == 400;
    call(client, base, "PUT", "/api/titles/1/watched", Map.of("value", true));
    assert call(client, base, "GET", "/api/library?status=watched", null).path("items").size() == 1;
    call(client, base, "PUT", "/api/titles/1/watched", Map.of("value", false));
    assert db.query("SELECT * FROM progress WHERE profile_id=1").isEmpty();
    db.execute("UPDATE titles SET hidden=1 WHERE id=1");
    assert call(client, base, "GET", "/api/library", null).path("items").isEmpty();
    assert AccountProgressTest.request(
                client, base, "PUT", "/api/titles/1/status", Map.of("status", "watched"))
            .statusCode()
        == 404;
    db.execute("UPDATE titles SET hidden=0 WHERE id=1");
    System.out.println(
        "PASS: local catalogue, Russian search, status/filter counts, personalized ratings, resume,"
            + " visibility.");
  }
}
