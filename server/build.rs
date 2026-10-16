// Интерфейс встраивается в программу из web/build. Без сборки фронта — заглушка, чтобы сервер
// собирался и работал отдельно (`cd web && npm run build` — настоящий интерфейс).
fn main() {
    let build = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/build");
    if !build.join("index.html").exists() {
        std::fs::create_dir_all(&build).expect("web/build");
        std::fs::write(
            build.join("index.html"),
            "<!doctype html><meta charset=utf-8><title>Films &amp; Serials For You</title><p>Интерфейс не собран: <code>cd web &amp;&amp; npm run build</code>",
        )
        .expect("заглушка");
    }
    println!("cargo:rerun-if-changed=../web/build");
}
