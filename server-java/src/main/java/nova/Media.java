package nova;

import com.sun.net.httpserver.HttpExchange;
import java.io.IOException;
import java.net.URLEncoder;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.Locale;

/** Bounded-memory direct streaming; each viewer owns a channel, never a whole video buffer. */
public final class Media {
  private Media() {}

  public static long[] range(String header, long size) {
    if (header == null || !header.startsWith("bytes=")) return null;
    String first = header.substring(6).split(",", 2)[0].strip();
    String[] parts = first.split("-", -1);
    if (parts.length != 2) return null;
    try {
      if (size == 0) return new long[] {-1, -1};
      long start, end;
      if (parts[0].isEmpty()) {
        long suffix = Long.parseLong(parts[1]);
        if (suffix <= 0) return new long[] {-1, -1};
        start = Math.max(0, size - suffix);
        end = size - 1;
      } else {
        start = Long.parseLong(parts[0]);
        end = parts[1].isEmpty() ? size - 1 : Math.min(Long.parseLong(parts[1]), size - 1);
      }
      return start < 0 || start > end || start >= size
          ? new long[] {-1, -1}
          : new long[] {start, end};
    } catch (NumberFormatException invalid) {
      return null;
    }
  }

  public static String mime(Path path) {
    String name = path.getFileName().toString().toLowerCase(Locale.ROOT);
    String ext = name.substring(name.lastIndexOf('.') + 1);
    return switch (ext) {
      case "mp4", "m4v" -> "video/mp4";
      case "webm" -> "video/webm";
      case "mov" -> "video/quicktime";
      case "mkv" -> "video/x-matroska";
      case "ts", "m2ts" -> "video/mp2t";
      case "jpg", "jpeg" -> "image/jpeg";
      case "png" -> "image/png";
      case "webp" -> "image/webp";
      case "vtt" -> "text/vtt; charset=utf-8";
      default -> "application/octet-stream";
    };
  }

  public static void send(HttpExchange exchange, Path path, boolean download) throws IOException {
    try (var channel = FileChannel.open(path, StandardOpenOption.READ)) {
      long size = channel.size();
      long[] range = range(exchange.getRequestHeaders().getFirst("Range"), size);
      var headers = exchange.getResponseHeaders();
      headers.set("Accept-Ranges", "bytes");
      headers.set("Content-Type", mime(path));
      if ("no-store".equals(headers.getFirst("Cache-Control"))
          || headers.getFirst("Cache-Control") == null)
        headers.set("Cache-Control", "private, max-age=3600");
      if (download)
        headers.set(
            "Content-Disposition",
            "attachment; filename*=UTF-8''"
                + URLEncoder.encode(path.getFileName().toString(), StandardCharsets.UTF_8)
                    .replace("+", "%20"));
      if (range != null && range[0] < 0) {
        headers.set("Content-Range", "bytes */" + size);
        exchange.sendResponseHeaders(416, -1);
        return;
      }
      long start = range == null ? 0 : range[0];
      long count = range == null ? size : range[1] - start + 1;
      if (range != null)
        headers.set("Content-Range", "bytes " + start + "-" + range[1] + "/" + size);
      headers.set("Content-Length", Long.toString(count));
      int status = range == null ? 200 : 206;
      if (exchange.getRequestMethod().equals("HEAD") || count == 0) {
        exchange.sendResponseHeaders(status, -1);
        return;
      }
      exchange.sendResponseHeaders(status, count);
      channel.position(start);
      byte[] data = new byte[64 * 1024];
      var buffer = ByteBuffer.wrap(data);
      try (var output = exchange.getResponseBody()) {
        while (count > 0) {
          buffer.clear().limit((int) Math.min(count, data.length));
          int read = channel.read(buffer);
          if (read < 0) throw new IOException("Видео изменилось во время чтения");
          output.write(data, 0, read);
          count -= read;
        }
      }
    }
  }
}
