#!/usr/bin/env bash
# Кладёт ffmpeg и ffprobe в desktop/src-tauri/resources для установщика.
# Использование: scripts/fetch-ffmpeg.sh macos-arm64 | macos-x64 | windows
# Не скачалось — выпуск собирается без ffmpeg (приложение подскажет, как поставить его самому).
set -uo pipefail
target="${1:?платформа: macos-arm64, macos-x64 или windows}"
out="$(cd "$(dirname "$0")/.." && pwd)/desktop/src-tauri/resources"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$out"

fetch() { curl -fsSL --retry 3 --retry-delay 5 -o "$2" "$1"; }

sha_ok() { # файл, ожидаемый хеш
  local got
  got="$( (sha256sum "$1" 2>/dev/null || shasum -a 256 "$1") | cut -d' ' -f1)"
  [ "$got" = "$(echo "$2" | tr -d '[:space:]' | cut -c1-64)" ]
}

case "$target" in
  macos-arm64|macos-x64)
    arch="${target#macos-}"; [ "$arch" = x64 ] && arch=amd64
    for tool in ffmpeg ffprobe; do
      url="https://ffmpeg.martin-riedl.de/redirect/latest/macos/${arch}/release/${tool}.zip"
      if ! fetch "$url" "$tmp/$tool.zip"; then echo "::warning::$tool не скачался ($url)"; exit 0; fi
      if fetch "$url.sha256" "$tmp/$tool.sha256" && ! sha_ok "$tmp/$tool.zip" "$(cat "$tmp/$tool.sha256")"; then
        echo "::error::$tool: SHA-256 не совпал"; exit 1
      fi
      unzip -oq "$tmp/$tool.zip" -d "$tmp/$tool"
      install -m 755 "$(find "$tmp/$tool" -type f -name "$tool" | head -1)" "$out/$tool"
    done
    ;;
  windows)
    url="https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    if ! fetch "$url" "$tmp/ff.zip"; then echo "::warning::ffmpeg не скачался ($url)"; exit 0; fi
    fetch "$url.sha256" "$tmp/ff.sha256" || { echo "::error::нет контрольной суммы"; exit 1; }
    sha_ok "$tmp/ff.zip" "$(cat "$tmp/ff.sha256")" || { echo "::error::ffmpeg: SHA-256 не совпал"; exit 1; }
    unzip -oq "$tmp/ff.zip" -d "$tmp/ff"
    for tool in ffmpeg ffprobe; do
      cp "$(find "$tmp/ff" -type f -name "$tool.exe" | head -1)" "$out/$tool.exe"
    done
    ;;
  *) echo "неизвестная платформа: $target"; exit 2 ;;
esac
ls -la "$out"
