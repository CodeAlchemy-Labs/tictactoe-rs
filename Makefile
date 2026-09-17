# Developer entry points. Targets are intentionally thin wrappers around
# `cargo`, `docker`, and `docker compose`.

SHELL := /bin/sh

.DEFAULT_GOAL := help

.PHONY: help build test lint fmt doc coverage clean demo demo-down \
        attach-client-1 attach-client-2 run-server run-hacker

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
		"  demo               build the image and start server + clients" \
		"  demo-down          stop the demo stack" \
		"  attach-client-1    attach to client-1 TUI" \
		"  attach-client-2    attach to client-2 TUI" \
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
	docker compose up -d server client-1 client-2
	@printf '\n'
	@printf 'Stack is up. Attach to a client with:\n'
	@printf '  make attach-client-1\n'
	@printf '  make attach-client-2\n'
	@printf '\n'
	@printf 'Run the hacker scenario with:\n'
	@printf '  docker compose --profile demo run --rm hacker\n'

demo-down:
	docker compose down --remove-orphans

attach-client-1:
	docker compose attach client-1

attach-client-2:
	docker compose attach client-2

run-server:
	cargo run --release --bin server

run-hacker:
	cargo run --release --bin hacker -- --target ws://127.0.0.1:8080/ws --scenario all