BINDIR := $(or $(XDG_BIN_HOME),$(XDG_BIN_DIR),$(HOME)/.local/bin)

.PHONY: all check test fmt clippy ci install

all: fmt check test

ci: fmt
	@cargo check --quiet 2>&1 || true
	@cargo clippy --quiet -- -D warnings 2>&1 || true
	@cargo test --quiet 2>&1 | grep -E '(^test |^running|test result|FAILED|error)'

check:
	@cargo check --quiet
	@cargo clippy --quiet -- -D warnings

test:
	@cargo test --quiet

fmt:
	@cargo fmt

clippy:
	@cargo clippy --quiet -- -D warnings

install:
	cargo build --release --quiet
	install -Dm755 target/release/td "$(BINDIR)/td"
