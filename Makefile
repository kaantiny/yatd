BINDIR := $(or $(XDG_BIN_HOME),$(XDG_BIN_DIR),$(HOME)/.local/bin)
JJ_FIX := jj --config 'fix.tools.rustfmt.command=["rustfmt","--emit","stdout","--edition","2021"]' --config 'fix.tools.rustfmt.patterns=["glob:**/*.rs"]' fix

.PHONY: all check test fmt clippy verify install clean release

all: fmt check test

verify:
	@$(JJ_FIX)
	@out=$$(cargo check --quiet 2>&1) || { printf '%s\n' "$$out"; exit 1; }; echo '✓ check'
	@out=$$(cargo clippy --quiet -- -D warnings 2>&1) || { printf '%s\n' "$$out"; exit 1; }; echo '✓ clippy'
	@out=$$(cargo test --quiet 2>&1) || { printf '%s\n' "$$out"; exit 1; }; echo '✓ tests'

check:
	@cargo check --quiet
	@cargo clippy --quiet -- -D warnings

test:
	@cargo test --quiet

fmt:
	@$(JJ_FIX)

clippy:
	@cargo clippy --quiet -- -D warnings

install:
	cargo build --release --quiet
	install -Dm755 target/release/td "$(BINDIR)/td"

clean:
	rm -rf dist

release:
	./release.fish
