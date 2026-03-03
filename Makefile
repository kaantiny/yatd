BINDIR  := $(or $(XDG_BIN_HOME),$(XDG_BIN_DIR),$(HOME)/.local/bin)
JJ_FIX  := jj --config 'fix.tools.rustfmt.command=["rustfmt","--emit","stdout","--edition","2021"]' --config 'fix.tools.rustfmt.patterns=["glob:**/*.rs"]' fix
VERSION := $(shell git describe --tags --always 2>/dev/null || echo v0.0.0)

TARGETS := \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu \
	x86_64-apple-darwin \
	aarch64-apple-darwin \
	x86_64-pc-windows-gnu \
	x86_64-unknown-freebsd

# UPX 5.x dropped macOS support; Windows and FreeBSD are limited to 32-bit
# formats. Only Linux targets are packable.
PACK_TARGETS := \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu

.PHONY: all check test fmt clippy verify install clean
.PHONY: release release-build release-pack release-upload release-all

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

release-build:
	rm -rf dist
	mkdir -p dist
	@for target in $(TARGETS); do \
		echo "Building $$target..."; \
		ext=""; case $$target in *windows*) ext=".exe";; esac; \
		cargo zigbuild --target $$target --release || exit 1; \
		cp "target/$$target/release/td$$ext" "dist/td-$(VERSION)-$$target$$ext"; \
	done

release-pack:
	@for target in $(PACK_TARGETS); do \
		bin="dist/td-$(VERSION)-$$target"; \
		if [ -f "$$bin" ]; then \
			echo "Packing $$target..."; \
			upx -q "$$bin"; \
		fi; \
	done

release-upload:
	fish -c 'release upload yatd $(VERSION) --latest dist/*'

release-all: release release-build release-pack release-upload

release:
	@set -e; \
	BUMP=$$(gum choose "major" "minor" "patch" "prerelease"); \
	CURRENT=$$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0"); \
	\
	IS_CURRENT_PRE=no; \
	echo "$$CURRENT" | grep -qE '\-[a-zA-Z]+\.[0-9]+$$' && IS_CURRENT_PRE=yes; \
	\
	IS_PRE=no; \
	if [ "$$BUMP" = "prerelease" ]; then \
		IS_PRE=yes; \
	else \
		gum confirm "Create pre-release?" && IS_PRE=yes || true; \
	fi; \
	\
	PRE_SUFFIX=""; \
	if [ "$$BUMP" = "prerelease" ] && [ "$$IS_CURRENT_PRE" = "yes" ]; then \
		PRE_SUFFIX=$$(echo "$$CURRENT" | sed -E 's/.*-([a-zA-Z]+)\.[0-9]+$$/\1/'); \
	elif [ "$$IS_PRE" = "yes" ]; then \
		PRE_SUFFIX=$$(gum input --placeholder "Pre-release suffix (e.g. beta, rc)"); \
	fi; \
	\
	if [ "$$BUMP" = "prerelease" ] && [ "$$IS_CURRENT_PRE" = "yes" ]; then \
		BASE_NEXT=$$(echo "$$CURRENT" | sed -E 's/-[a-zA-Z]+\.[0-9]+$$//'); \
	else \
		BASE_NEXT=$$(svu $$BUMP); \
	fi; \
	\
	SUFFIX_VER=""; \
	if [ "$$IS_PRE" = "yes" ] && [ -n "$$PRE_SUFFIX" ]; then \
		if [ "$$BUMP" = "prerelease" ] && [ "$$IS_CURRENT_PRE" = "yes" ]; then \
			cur_num=$$(echo "$$CURRENT" | sed -E 's/.*-[a-zA-Z]+\.([0-9]+)$$/\1/'); \
			SUFFIX_VER=$$((cur_num + 1)); \
		else \
			highest=$$(git tag -l "$$BASE_NEXT-$$PRE_SUFFIX.*" \
				| sed "s/.*-$$PRE_SUFFIX\.//" | sort -n | tail -1); \
			if [ -n "$$highest" ]; then \
				SUFFIX_VER=$$((highest + 1)); \
			else \
				SUFFIX_VER=0; \
			fi; \
		fi; \
	fi; \
	\
	if [ "$$IS_PRE" = "yes" ] && [ -n "$$PRE_SUFFIX" ]; then \
		NEXT="$$BASE_NEXT-$$PRE_SUFFIX.$$SUFFIX_VER"; \
	else \
		NEXT="$$BASE_NEXT"; \
	fi; \
	\
	PARENT=$$(jj log -r '@-' --no-graph -T 'bookmarks' 2>/dev/null); \
	case "$$PARENT" in \
		*main*|*dev*) ;; \
		*) echo "error: parent commit is not on main or dev" >&2; exit 1;; \
	esac; \
	if jj diff --summary 2>/dev/null | grep -qE '^[MD] '; then \
		echo "error: working copy has modified or deleted files" >&2; exit 1; \
	fi; \
	\
	gum confirm "Release $$NEXT?" || exit 1; \
	CARGO_VER=$$(echo "$$NEXT" | sed 's/^v//'); \
	sed -i "s/^version = \".*\"/version = \"$$CARGO_VER\"/" Cargo.toml; \
	jj commit -m "Bump version to $$NEXT"; \
	jj tag set "$$NEXT" -r @-; \
	git push soft "$$NEXT"; \
	jj git push --remote soft -b main; \
	echo "Tagged and pushed $$NEXT"
