package nova;

import java.net.http.HttpClient;
import java.util.Map;

final class MembersTest {
  private MembersTest() {}

  static void run(Database db, HttpClient client, String base) throws Exception {
    db.execute(
        "INSERT INTO users(id,login,name,pass,role,created_at)"
            + " VALUES(900,'member-test','Алёна','unused','viewer',0)");
    db.execute("INSERT INTO profiles(id,user_id,name,created_at) VALUES(900,900,'Алёна',0)");
    db.execute(
        "INSERT INTO ext_titles(id,kind,title,updated_at) VALUES('tt9999009','movie','Фильм',0)");
    db.execute(
        "INSERT INTO ext_marks(profile_id,ext_id,status,at) VALUES(900,'tt9999009','watched',0)");
    db.execute(
        "INSERT INTO titles(id,kind,key,title,imdb_id,added_at,updated_at)"
            + " VALUES(900,'movie','member-film','Фильм','tt9999009',0,0)");
    db.execute("INSERT INTO watched(profile_id,title_id,at) VALUES(900,900,0)");
    db.execute(
        "INSERT INTO reviews(title_id,profile_id,body,created_at) VALUES(900,900,'Отзыв',0)");
    db.execute("INSERT INTO friends(a,b,status,created_at) VALUES(900,1,'pending',0)");
    var rows =
        CatalogTest.call(
            client, base, "GET", "/api/members?q=%D0%B0%D0%BB%D0%B5%D0%BD%D0%B0", null);
    assert rows.size() == 1 : rows;
    assert rows.get(0).path("watched").asInt() == 1 : rows;
    assert rows.get(0).path("reviews").asInt() == 1 : rows;
    assert rows.get(0).path("relation").asText().equals("incoming") : rows;
    assert CatalogTest.call(client, base, "POST", "/api/friends/900", Map.of())
        .path("relation")
        .asText()
        .equals("friend");
    assert CatalogTest.call(client, base, "DELETE", "/api/friends/900", null)
        .path("relation")
        .asText()
        .equals("none");
    assert CatalogTest.call(client, base, "POST", "/api/friends/900", Map.of())
        .path("relation")
        .asText()
        .equals("outgoing");
    assert CatalogTest.call(client, base, "POST", "/api/friends/900", Map.of())
        .path("relation")
        .asText()
        .equals("outgoing");
    assert AccountProgressTest.request(client, base, "POST", "/api/friends/1", Map.of())
            .statusCode()
        == 400;
    db.execute("UPDATE users SET disabled=1 WHERE id=900");
    assert CatalogTest.call(client, base, "GET", "/api/members", null).isEmpty();
    assert AccountProgressTest.request(client, base, "POST", "/api/friends/900", Map.of())
            .statusCode()
        == 404;
    db.execute("DELETE FROM users WHERE id=900");
    db.execute("DELETE FROM titles WHERE id=900");
    db.execute("DELETE FROM ext_titles WHERE id='tt9999009'");
    System.out.println(
        "PASS: Java members, external watches deduplication, reviews, Russian search,"
            + " relationships, disabled accounts.");
  }
}
