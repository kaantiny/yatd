#!/usr/bin/env fish

function __release_yatd_usage
    echo "Usage: release.fish [--from <stage>] [--only <stage>] [--help]

Stages (run in order):
    tag       Bump Cargo.toml, commit, tag, and push
    build     Cross-compile for all release targets
    pack      Compress Linux binaries with UPX
    upload    Upload artifacts via release(1)

Flags:
    --from <stage>   Start at this stage and continue through the rest
    --only <stage>   Run just this one stage
    --help, -h       Show this help

With no flags, all stages run in order (tag → build → pack → upload).

Examples:
    ./release.fish                 Full release
    ./release.fish --from build    Rebuild, pack, and upload (skip tagging)
    ./release.fish --only build    Just cross-compile
    ./release.fish --only upload   Re-upload existing dist/ artifacts"
end

set -l targets \
    x86_64-unknown-linux-gnu \
    aarch64-unknown-linux-gnu \
    x86_64-apple-darwin \
    aarch64-apple-darwin \
    x86_64-pc-windows-gnu \
    x86_64-unknown-freebsd

# UPX 5.x dropped macOS support; Windows and FreeBSD are limited to 32-bit
# formats. Only Linux targets are packable.
set -l pack_targets \
    x86_64-unknown-linux-gnu \
    aarch64-unknown-linux-gnu

# Global so __should_run can see them. Prefixed to avoid collisions.
set -g __ry_stages tag build pack upload
set -g __ry_from ""
set -g __ry_only ""

# --- Parse flags ---

set -l i 1
while test $i -le (count $argv)
    switch $argv[$i]
        case --from
            set i (math $i + 1)
            set -g __ry_from $argv[$i]
        case --only
            set i (math $i + 1)
            set -g __ry_only $argv[$i]
        case --help -h
            __release_yatd_usage
            exit 0
        case '*'
            echo "Error: unknown flag '$argv[$i]'" >&2
            __release_yatd_usage >&2
            exit 1
    end
    set i (math $i + 1)
end

if test -n "$__ry_from" -a -n "$__ry_only"
    echo "Error: --from and --only are mutually exclusive" >&2
    exit 1
end

if test -n "$__ry_from"; and not contains -- "$__ry_from" $__ry_stages
    echo "Error: unknown stage '$__ry_from' (valid: $__ry_stages)" >&2
    exit 1
end

if test -n "$__ry_only"; and not contains -- "$__ry_only" $__ry_stages
    echo "Error: unknown stage '$__ry_only' (valid: $__ry_stages)" >&2
    exit 1
end

# Returns 0 if the given stage should execute, 1 otherwise.
function __should_run -a stage
    if test -n "$__ry_only"
        test "$stage" = "$__ry_only"
        return
    end

    if test -z "$__ry_from"
        return 0
    end

    set -l reached 0
    for s in $__ry_stages
        if test "$s" = "$__ry_from"
            set reached 1
        end
        if test $reached -eq 1 -a "$s" = "$stage"
            return 0
        end
    end
    return 1
end

# --- Resolve tag ---
#
# When skipping the tag stage we still need a tag for filenames and
# ldflags, so fall back to git describe.

set -l tag

if __should_run tag
    # --- Guards ---

    set -l parent_bookmarks (jj log -r '@-' --no-graph -T 'bookmarks' 2>/dev/null)
    if not string match -rq 'main|dev' -- "$parent_bookmarks"
        echo "Error: parent commit is not on main or dev" >&2
        exit 1
    end

    if jj diff --summary 2>/dev/null | grep -qE '^[MD] '
        echo "Error: working copy has modified or deleted files" >&2
        exit 1
    end

    # --- Fetch tags ---

    jj git fetch --remote soft

    # --- Compute next version ---

    # jj tag list output is "<tag>: <hash> <desc>"; extract the tag name.
    set -l current (jj tag list 2>/dev/null | awk '{print $1}' | string replace -r ':$' '' | sort -V | tail -1)
    if test -z "$current"
        set current v0.0.0
    end
    set -l bump (gum choose major minor patch prerelease)

    # Detect whether the current tag is already a prerelease
    # (e.g. v1.2.3-rc.4).
    set -l current_is_prerelease no
    if string match -rq -- '-[a-zA-Z]+\.[0-9]+$' $current
        set current_is_prerelease yes
    end

    # Ask whether to create a prerelease, unless the user already chose
    # "prerelease".
    set -l is_prerelease no
    if test "$bump" = prerelease
        set is_prerelease yes
    else
        if gum confirm "Create pre-release?"
            set is_prerelease yes
        end
    end

    # Determine the prerelease suffix (e.g. "beta", "rc").
    set -l prerelease_suffix ""
    if test "$bump" = prerelease -a "$current_is_prerelease" = yes
        # Reuse the suffix from the current prerelease tag.
        set prerelease_suffix (string replace -r '.*-([a-zA-Z]+)\.[0-9]+$' '$1' $current)
    else if test "$is_prerelease" = yes
        set prerelease_suffix (gum input --placeholder "Enter pre-release suffix (e.g. beta, rc)")
        if test -z "$prerelease_suffix"
            echo "Error: pre-release suffix is required" >&2
            exit 1
        end
    end

    # Compute the base version (without prerelease suffix).
    set -l base_next
    if test "$bump" = prerelease -a "$current_is_prerelease" = yes
        # Strip the prerelease suffix to get the base
        # (e.g. v1.2.3-rc.4 → v1.2.3).
        set base_next (string replace -r -- '-[a-zA-Z]+\.[0-9]+$' '' $current)
    else
        set base_next (svu $bump)
    end

    # Compute the suffix version number.
    set -l suffix_ver ""
    if test "$is_prerelease" = yes -a -n "$prerelease_suffix"
        if test "$bump" = prerelease -a "$current_is_prerelease" = yes
            # Increment the current prerelease number.
            set -l current_num (string replace -r '.*-[a-zA-Z]+\.([0-9]+)$' '$1' $current)
            set suffix_ver (math $current_num + 1)
        else
            # Find existing tags with this suffix and increment past the
            # highest.
            set -l highest (jj tag list 2>/dev/null \
                | awk '{print $1}' \
                | string replace -r ':$' '' \
                | string replace -r "^$base_next-$prerelease_suffix\\." '' \
                | string match -r '^\d+$' \
                | sort -n | tail -1)
            if test -n "$highest"
                set suffix_ver (math $highest + 1)
            else
                set suffix_ver 0
            end
        end
    end

    # Assemble the final tag.
    if test "$is_prerelease" = yes -a -n "$prerelease_suffix"
        set tag "$base_next-$prerelease_suffix.$suffix_ver"
    else
        set tag "$base_next"
    end

    # --- Confirm ---

    read -P "Release $tag? [y/N] " confirm
    if test "$confirm" != y -a "$confirm" != Y
        echo Aborted.
        exit 0
    end

    # --- Bump Cargo.toml and commit ---

    set -l cargo_ver (string replace -r '^v' '' $tag)
    sed -i "s/^version = \".*\"/version = \"$cargo_ver\"/" Cargo.toml
    jj commit -m "Bump version to $tag"; or begin
        echo "Error: jj commit failed" >&2
        exit 1
    end

    # --- Tag and push ---

    jj tag set $tag -r @-; or begin
        echo "Error: tagging failed" >&2
        exit 1
    end

    git push soft $tag; or begin
        echo "Error: tag push failed" >&2
        exit 1
    end

    jj git push --remote soft -b main; or begin
        echo "Error: branch push failed" >&2
        exit 1
    end

    echo "Tagged and pushed $tag"
else
    set tag (git describe --tags --always 2>/dev/null; or echo dev)
end

# --- Build ---

if __should_run build
    rm -rf dist
    mkdir -p dist

    for target in $targets
        echo "Building $target..."
        set -l ext ""
        if string match -q '*windows*' $target
            set ext .exe
        end
        cargo zigbuild --target $target --release; or begin
            echo "Error: build failed for $target" >&2
            exit 1
        end
        cp "target/$target/release/td$ext" "dist/td-$tag-$target$ext"
    end
end

# --- Pack ---

if __should_run pack
    for target in $pack_targets
        set -l bin "dist/td-$tag-$target"
        if test -f "$bin"
            echo "Packing $target..."
            upx -q "$bin"
        end
    end
end

# --- Upload ---

if __should_run upload
    fish -c "release upload yatd $tag --latest dist/*"
end
