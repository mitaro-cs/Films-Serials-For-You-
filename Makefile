.PHONY: dev web build test lint fmt desktop desktop-run

dev:            ## сервер на :17480 (данные в ./data-dev) + vite на :5173
	cargo run -p fsfy-server -- serve --data ./data-dev & cd web && npm run dev

web:
	cd web && npm ci && npm run build

build: web      ## сервер с встроенным интерфейсом: target/release/fsfy
	cargo build --release -p fsfy-server

test:
	cargo test -p fsfy-server
	cd web && npm test

lint:
	cargo fmt --all --check
	cargo clippy -p fsfy-server --all-targets -- -D warnings
	cd web && npm run check

fmt:
	cargo fmt --all

desktop: web    ## установщик для этой ОС
	cd desktop && npm ci && npm run tauri build

desktop-run: web
	cargo run -p fsfy-desktop
