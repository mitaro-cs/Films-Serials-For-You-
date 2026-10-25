package nova;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.net.*;
import java.net.http.*;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Set;
import java.util.concurrent.*;

/** Cache format is shared with the current server. No remote call on a UI request. */
public final class EpisodeLocale implements AutoCloseable {
  private final Database db;
  private final ExecutorService executor = Executors.newVirtualThreadPerTaskExecutor();
  private final Semaphore network = new Semaphore(2);
  private final Set<String> pending = ConcurrentHashMap.newKeySet();
  private final HttpClient http =
      HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5)).build();

  public EpisodeLocale(Database db) {
    this.db = db;
  }

  public static boolean russian(String text) {
    return text != null && text.matches("(?s).*[А-Яа-яЁё].*");
  }

  public static String key(String text) {
    return "episode.ru:" + Passwords.sha256(text.strip());
  }

  public String localized(String text) throws Exception {
    if (text == null || text.isBlank()) return null;
    if (russian(text)
        || db.setting("meta.language", "ru").equals("source")
        || text.codePoints().noneMatch(Character::isLetter)) return text;
    String cached = db.setting(key(text), "");
    return russian(cached) ? cached : null;
  }

  public void queue(String text) throws Exception {
    if (text == null
        || localized(text) != null
        || text.getBytes(StandardCharsets.UTF_8).length > 450) return;
    String key = key(text);
    if (System.currentTimeMillis() - Long.parseLong(db.setting(key + ".at", "0")) < 86400000
        || !pending.add(key)) return;
    executor.submit(
        () -> {
          boolean acquired = false;
          try {
            network.acquire();
            acquired = true;
            db.setSetting(key + ".at", Long.toString(System.currentTimeMillis()));
            String url =
                "https://api.mymemory.translated.net/get?langpair=en%7Cru&q="
                    + URLEncoder.encode(text, StandardCharsets.UTF_8);
            var response =
                http.send(
                    HttpRequest.newBuilder(URI.create(url))
                        .timeout(Duration.ofSeconds(8))
                        .GET()
                        .build(),
                    HttpResponse.BodyHandlers.ofString());
            if (response.statusCode() != 200) return;
            var value = new ObjectMapper().readTree(response.body());
            String translated = value.path("responseData").path("translatedText").asText();
            if (value.path("responseStatus").asInt() == 200
                && russian(translated)
                && !translated.toUpperCase().contains("MYMEMORY")) db.setSetting(key, translated);
          } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
          } catch (Exception ignored) {
            /* Keep source metadata and retry after the cooldown. */
          } finally {
            if (acquired) network.release();
            pending.remove(key);
          }
        });
  }

  @Override
  public void close() {
    executor.shutdownNow();
    http.close();
  }
}
