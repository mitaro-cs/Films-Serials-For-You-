package nova;

import java.nio.charset.StandardCharsets;
import java.security.*;
import java.util.*;
import org.bouncycastle.crypto.generators.Argon2BytesGenerator;
import org.bouncycastle.crypto.params.Argon2Parameters;

/** Argon2id PHC compatibility with existing Rust-created accounts. */
public final class Passwords {
  private static final java.util.concurrent.Semaphore HASH_SLOTS =
      new java.util.concurrent.Semaphore(2, true);

  private Passwords() {}

  public static String hash(String password) {
    byte[] salt = new byte[16];
    new SecureRandom().nextBytes(salt);
    byte[] result = derive(password, salt, 19456, 2, 1, 32);
    var b64 = Base64.getEncoder().withoutPadding();
    return "$argon2id$v=19$m=19456,t=2,p=1$"
        + b64.encodeToString(salt)
        + "$"
        + b64.encodeToString(result);
  }

  public static boolean verify(String password, String phc) {
    if (password == null || password.length() > 1024 || phc == null) return false;
    try {
      var fields = phc.split("\\$");
      if (fields.length != 6 || !fields[1].equals("argon2id") || !fields[2].equals("v=19"))
        return false;
      var options = new HashMap<String, Integer>();
      for (var pair : fields[3].split(",")) {
        var bits = pair.split("=");
        options.put(bits[0], Integer.parseInt(bits[1]));
      }
      int memory = options.get("m"), iterations = options.get("t"), parallelism = options.get("p");
      if (memory < 8
          || memory > 262144
          || iterations < 1
          || iterations > 10
          || parallelism < 1
          || parallelism > 16) return false;
      byte[] expected = Base64.getDecoder().decode(fields[5]);
      if (expected.length < 16 || expected.length > 64) return false;
      return MessageDigest.isEqual(
          expected,
          derive(
              password,
              Base64.getDecoder().decode(fields[4]),
              memory,
              iterations,
              parallelism,
              expected.length));
    } catch (RuntimeException error) {
      return false;
    }
  }

  private static byte[] derive(
      String password, byte[] salt, int memory, int iterations, int parallelism, int length) {
    try {
      HASH_SLOTS.acquire();
    } catch (InterruptedException error) {
      Thread.currentThread().interrupt();
      throw new IllegalStateException("Password operation interrupted", error);
    }
    try {
      var parameters =
          new Argon2Parameters.Builder(Argon2Parameters.ARGON2_id)
              .withVersion(19)
              .withSalt(salt)
              .withMemoryAsKB(memory)
              .withIterations(iterations)
              .withParallelism(parallelism)
              .build();
      var generator = new Argon2BytesGenerator();
      generator.init(parameters);
      byte[] result = new byte[length];
      generator.generateBytes(password.getBytes(StandardCharsets.UTF_8), result);
      return result;
    } finally {
      HASH_SLOTS.release();
    }
  }

  public static String sha256(String text) {
    try {
      return HexFormat.of()
          .formatHex(
              MessageDigest.getInstance("SHA-256").digest(text.getBytes(StandardCharsets.UTF_8)));
    } catch (NoSuchAlgorithmException error) {
      throw new IllegalStateException(error);
    }
  }

  public static String token() {
    byte[] bytes = new byte[32];
    new SecureRandom().nextBytes(bytes);
    return Base64.getUrlEncoder().withoutPadding().encodeToString(bytes);
  }
}
