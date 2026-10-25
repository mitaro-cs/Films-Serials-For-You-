package nova;

import java.util.*;
import java.util.concurrent.*;

/** In-memory room state, ordered commands, bounded SSE queues, no pause on disconnect. */
public final class Rooms {
  private final ConcurrentMap<String, Room> rooms = new ConcurrentHashMap<>();

  public static final class Subscription implements AutoCloseable {
    final Room room;
    final long profile;
    final BlockingQueue<Map<String, Object>> events = new ArrayBlockingQueue<>(128);

    Subscription(Room room, long profile) {
      this.room = room;
      this.profile = profile;
    }

    public Map<String, Object> next(long timeoutMillis) throws InterruptedException {
      return events.poll(timeoutMillis, TimeUnit.MILLISECONDS);
    }

    @Override
    public void close() {
      room.leave(this);
    }
  }

  public static final class Room {
    final String id;
    final long owner, titleId;
    long fileId;
    final Long linkId;
    long revision = 0, at = System.currentTimeMillis(), emptySince = at;
    boolean playing = false;
    double position;
    String by;
    final Map<Long, Map<String, Object>> members = new TreeMap<>();
    final Set<Subscription> subscribers = new HashSet<>();
    final ArrayDeque<Map<String, Object>> chat = new ArrayDeque<>();

    Room(
        String id, long owner, long titleId, long fileId, Long linkId, double position, String by) {
      this.id = id;
      this.owner = owner;
      this.titleId = titleId;
      this.fileId = fileId;
      this.linkId = linkId;
      this.position = position;
      this.by = by;
    }

    synchronized Map<String, Object> state() {
      var state = new LinkedHashMap<String, Object>();
      state.put("revision", revision);
      state.put("fileId", fileId);
      state.put("linkId", linkId);
      state.put("playing", playing);
      long now = System.currentTimeMillis();
      state.put("position", position + (playing ? Math.max(0, now - at) / 1000.0 : 0));
      state.put("at", now);
      state.put("by", by);
      return state;
    }

    public synchronized Map<String, Object> snapshot() {
      return Map.of(
          "type",
          "snapshot",
          "id",
          id,
          "ownerId",
          owner,
          "titleId",
          titleId,
          "state",
          state(),
          "members",
          List.copyOf(members.values()),
          "chat",
          List.copyOf(chat),
          "voice",
          List.of());
    }

    public synchronized Subscription join(Map<String, Object> who) {
      long profile = ((Number) who.get("id")).longValue();
      var sub = new Subscription(this, profile);
      subscribers.add(sub);
      members.put(profile, who);
      emptySince = 0;
      send(Map.of("type", "members", "members", List.copyOf(members.values())));
      sub.events.clear();
      sub.events.offer(snapshot());
      return sub;
    }

    synchronized void leave(Subscription sub) {
      if (!subscribers.remove(sub)) return;
      if (subscribers.stream().noneMatch(s -> s.profile == sub.profile))
        members.remove(sub.profile);
      if (subscribers.isEmpty()) emptySince = System.currentTimeMillis();
      send(Map.of("type", "members", "members", List.copyOf(members.values())));
    }

    public synchronized Map<String, Object> command(
        long file, boolean play, double time, String name) {
      if (!Double.isFinite(time)) throw new IllegalArgumentException("Неверная позиция");
      fileId = file;
      playing = play;
      position = Math.max(0, time);
      at = System.currentTimeMillis();
      by = name;
      revision++;
      var state = state();
      send(Map.of("type", "state", "state", state));
      return state;
    }

    public synchronized void say(Map<String, Object> who, String text) {
      if (text.isBlank() || text.length() > 2000)
        throw new IllegalArgumentException("Длина сообщения: от 1 до 2000 знаков");
      var message =
          Map.<String, Object>of("from", who, "body", text, "at", System.currentTimeMillis());
      chat.addLast(message);
      while (chat.size() > 60) chat.removeFirst();
      var event = new LinkedHashMap<>(message);
      event.put("type", "chat");
      send(event);
    }

    public synchronized void react(Map<String, Object> who, String emoji) {
      if (!List.of("❤️", "😂", "😮", "😢", "🔥", "👏", "😱", "👍").contains(emoji))
        throw new IllegalArgumentException("Неизвестная реакция");
      send(Map.of("type", "react", "from", who, "emoji", emoji));
    }

    private void send(Map<String, Object> event) {
      for (var sub : subscribers)
        if (!sub.events.offer(event)) {
          sub.events.clear();
          sub.events.offer(snapshot());
        }
    }
  }

  public String create(long owner, long title, long file, Long link, double position, String by) {
    sweep();
    String id = Passwords.token().substring(0, 12).toLowerCase(Locale.ROOT);
    rooms.put(id, new Room(id, owner, title, file, link, position, by));
    return id;
  }

  public Room get(String id) {
    return rooms.get(id);
  }

  public void sweep() {
    long now = System.currentTimeMillis();
    rooms
        .values()
        .removeIf(
            r -> {
              synchronized (r) {
                return r.emptySince != 0 && now - r.emptySince > 600000;
              }
            });
  }
}
