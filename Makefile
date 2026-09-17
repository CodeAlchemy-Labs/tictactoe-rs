# Developer entry points. Targets are intentionally thin wrappers around
# `cargo`, `docker`, and `docker compose`.

SHELL := /bin/sh

.DEFAULT_GOAL := help

.PHONY: help build test lint fmt doc coverage clean demo demo-down \
        client-1 client-2 hacker run-server run-hacker

help:
	@printf '%s\n' \
		"Targets:" \
		"  build              cargo build --release --workspace" \
		"  test               cargo test --workspace" \
		"  lint               cargo clippy --workspace --all-targets -- -D warnings" \
		"  fmt                cargo fmt --all -- --check" \
		"  doc                cargo doc --workspace --no-deps" \
		"  coverage           cargo llvm-cov --workspace --html" \
		"  clean              remove build artifacts and docker resources" \
		"  demo               build the image and start the server" \
		"  demo-down          stop the demo stack" \
		"  client-1           run client-1 in the foreground (interactive TUI)" \
		"  client-2           run client-2 in the foreground (interactive TUI)" \
		"  hacker             run all hacker scenarios against the running server" \
		"  run-server         run the server locally" \
		"  run-hacker         run the hacker locally against a running server"

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
	cargo llvm-cov --workspace --html

clean:
	cargo clean
	docker compose down --rmi local --volumes --remove-orphans

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