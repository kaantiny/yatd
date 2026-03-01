BINDIR := $(or $(XDG_BIN_HOME),$(XDG_BIN_DIR),$(HOME)/.local/bin)

.PHONY: all check test fmt clippy verify install

all: fmt check test

verify:
	@jj fix
	@out=$$(cargo check --quiet 2>&1) || { printf '%s\n' "$$out"; exit 1; }; echo '✓ check'
	@out=$$(cargo clippy --quiet -- -D warnings 2>&1) || { printf '%s\n' "$$out"; exit 1; }; echo '✓ clippy'
	@out=$$(cargo test --quiet 2>&1) || { printf '%s\n' "$$out"; exit 1; }; echo '✓ tests'

check:
	@cargo check --quiet
	@cargo clippy --quiet -- -D warnings

test:
	@cargo test --quiet

fmt:
	@jj fix

clippy:
	@cargo clippy --quiet -- -D warnings

install:
	cargo build --release --quiet
	install -Dm755 target/release/td "$(BINDIR)/td"
