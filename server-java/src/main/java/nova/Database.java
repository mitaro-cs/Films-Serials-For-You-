package nova;

import java.nio.file.*;
import java.sql.*;
import java.util.*;
import java.util.concurrent.locks.ReentrantLock;
import org.sqlite.SQLiteConfig;

/** The same SQLite schema and migration versions as the released server. */
public final class Database implements AutoCloseable {
  @FunctionalInterface
  public interface Transaction<T> {
    T run() throws Exception;
  }

  public <T> T transaction(Transaction<T> work) throws Exception {
    lock.lock();
    try {
      if (!connection.getAutoCommit()) throw new SQLException("Вложенная транзакция");
      connection.setAutoCommit(false);
      try {
        T value = work.run();
        connection.commit();
        return value;
      } catch (Exception error) {
        connection.rollback();
        throw error;
      } finally {
        connection.setAutoCommit(true);
      }
    } finally {
      lock.unlock();
    }
  }

  private final Connection connection;
  final Path directory;
  private final ReentrantLock lock = new ReentrantLock();

  public Database(Path path) throws Exception {
    directory = path.toAbsolutePath().normalize().getParent();
    Files.createDirectories(path.toAbsolutePath().getParent());
    SQLiteConfig config = new SQLiteConfig();
    config.enforceForeignKeys(true);
    config.setBusyTimeout(5000);
    connection =
        DriverManager.getConnection("jdbc:sqlite:" + path.toAbsolutePath(), config.toProperties());
    try {
      query("PRAGMA journal_mode=WAL");
      execute("PRAGMA synchronous=NORMAL");
      int version =
          ((Number) query("PRAGMA user_version").getFirst().values().iterator().next()).intValue();
      if (version > 18) throw new IllegalStateException("База от более новой версии Nova");
      for (int i = version + 1; i <= 18; i++) {
        try (var stream = Database.class.getResourceAsStream("/migrations/%03d.sql".formatted(i))) {
          if (stream == null) throw new IllegalStateException("Нет миграции " + i);
          var sql = new String(stream.readAllBytes(), java.nio.charset.StandardCharsets.UTF_8);
          var sqlite = (org.sqlite.SQLiteConnection) connection;
          connection.setAutoCommit(false);
          try {
            int code = sqlite.getDatabase()._exec(sql);
            if (code != 0) throw new SQLException("Миграция " + i + ": SQLite " + code);
            execute("PRAGMA user_version=" + i);
            connection.commit();
          } catch (Exception error) {
            connection.rollback();
            throw error;
          } finally {
            connection.setAutoCommit(true);
          }
        }
      }
    } catch (Exception error) {
      connection.close();
      throw error;
    }
  }

  public List<Map<String, Object>> query(String sql, Object... args) throws SQLException {
    lock.lock();
    try (var statement = connection.prepareStatement(sql)) {
      bind(statement, args);
      try (var result = statement.executeQuery()) {
        var rows = new ArrayList<Map<String, Object>>();
        var meta = result.getMetaData();
        while (result.next()) {
          var row = new LinkedHashMap<String, Object>();
          for (int i = 1; i <= meta.getColumnCount(); i++)
            row.put(meta.getColumnLabel(i), result.getObject(i));
          rows.add(row);
        }
        return rows;
      }
    } finally {
      lock.unlock();
    }
  }

  public int execute(String sql, Object... args) throws SQLException {
    lock.lock();
    try (var statement = connection.prepareStatement(sql)) {
      bind(statement, args);
      return statement.executeUpdate();
    } finally {
      lock.unlock();
    }
  }

  private static void bind(PreparedStatement statement, Object[] args) throws SQLException {
    for (int i = 0; i < args.length; i++) statement.setObject(i + 1, args[i]);
  }

  public String setting(String key, String fallback) throws SQLException {
    var rows = query("SELECT value FROM settings WHERE key=?", key);
    return rows.isEmpty() ? fallback : String.valueOf(rows.getFirst().get("value"));
  }

  public void setSetting(String key, String value) throws SQLException {
    execute(
        "INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET"
            + " value=excluded.value",
        key,
        value);
  }

  @Override
  public void close() throws SQLException {
    connection.close();
  }
}
