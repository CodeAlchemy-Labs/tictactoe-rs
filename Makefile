# Developer entry points. Targets are intentionally thin wrappers around
# `cargo`, `docker`, and `docker compose`.

SHELL := /bin/sh

.DEFAULT_GOAL := help

.PHONY: help tools build test lint fmt doc coverage clean demo demo-down \
        client-1 client-2 hacker run-server run-hacker \
        prod-build prod-up prod-down prod-logs

help:
	@printf '%s\n' \
		"Targets:" \
		"  tools              install development tools (cargo-llvm-cov)" \
		"  build              cargo build --release --workspace" \
		"  test               cargo test --workspace" \
		"  lint               cargo clippy --workspace --all-targets -- -D warnings" \
		"  fmt                cargo fmt --all -- --check" \
		"  doc                cargo doc --workspace --no-deps" \
		"  coverage           cargo llvm-cov --workspace --html (needs 'make tools')" \
		"  clean              remove build artifacts and docker resources" \
		"  demo               build the image and start the server" \
		"  demo-down          stop the demo stack" \
		"  client-1           run client-1 in the foreground (interactive TUI)" \
		"  client-2           run client-2 in the foreground (interactive TUI)" \
		"  hacker             run all hacker scenarios against the running server" \
		"  run-server         run the server locally" \
		"  run-hacker         run the hacker locally against a running server" \
		"" \
		"Production image targets (docker-compose.prod.yml):" \
		"  prod-build         build the server-only production image" \
		"  prod-up            start the production stack on localhost:8080" \
		"  prod-down          stop the production stack" \
		"  prod-logs          tail production server logs"

tools:
	@command -v cargo-llvm-cov >/dev/null 2>&1 || cargo install cargo-llvm-cov --locked
	@command -v rustup >/dev/null 2>&1 && rustup component add llvm-tools-preview || true

build:
	cargo build --release --workspace

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all -- --check

doc:
	cargo doc --workspace --no-deps

coverage:
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { \
		printf 'cargo-llvm-cov is not installed. Run: make tools\n' >&2; \
		exit 1; \
	}
	cargo llvm-cov --workspace --html

clean:
	cargo clean
	docker compose --profile interactive --profile demo down --rmi local --volumes --remove-orphans

demo:
	docker compose build
	docker compose up -d server
	@printf '\n'
	@printf 'Server is up. In two separate terminals, run:\n'
	@printf '  make client-1\n'
	@printf '  make client-2\n'
	@printf '\n'
	@printf 'Then, from a third terminal, run the hacker scenarios:\n'
	@printf '  make hacker\n'
	@printf '\n'
	@printf 'Stop the stack with:\n'
	@printf '  make demo-down\n'

demo-down:
	docker compose --profile interactive --profile demo down --remove-orphans

client-1:
	docker compose --profile interactive run --rm -it client-1

client-2:
	docker compose --profile interactive run --rm -it client-2

hacker:
	docker compose --profile demo run --rm hacker

run-server:
	cargo run --release --bin server

run-hacker:
	cargo run --release --bin hacker -- --target ws://127.0.0.1:8080/ws --scenario all
prod-build:
	docker compose -f docker-compose.prod.yml build

prod-up:
	docker compose -f docker-compose.prod.yml up -d
	@printf '\n'
	@printf 'Production server is up at ws://127.0.0.1:8080/ws\n'
	@printf 'Smoke-test with:\n'
	@printf '  cargo run --release --bin tictacli -- --server ws://127.0.0.1:8080/ws --name smoke\n'
	@printf '\n'
	@printf 'Stop with:\n'
	@printf '  make prod-down\n'

prod-down:
	docker compose -f docker-compose.prod.yml down --remove-orphans

prod-logs:
	docker compose -f docker-compose.prod.yml logs -f server
