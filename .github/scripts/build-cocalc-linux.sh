#!/usr/bin/env bash
set -euo pipefail

# Run on the dedicated Linux build VM, with Zig 0.14 and musl GCC on PATH.
# ARM64 cross-builds also require aarch64-linux-musl-gcc and cross binutils.
target="${1:?usage: build-cocalc-linux.sh TARGET}"
case "$target" in
  x86_64-unknown-linux-musl) arch=x64; strip_tool=strip ;;
  aarch64-unknown-linux-musl) arch=arm64; strip_tool=aarch64-linux-gnu-strip ;;
  *) echo "unsupported target: $target" >&2; exit 1 ;;
esac
root="$(cd "$(dirname "$0")/../.." && pwd)"
export TARGET="$target"
export RUNNER_TEMP="${RUNNER_TEMP:-/tmp/cocalc-codex-0159}"
mkdir -p "$RUNNER_TEMP"
export GITHUB_ENV="$RUNNER_TEMP/$target.env"
: > "$GITHUB_ENV"
bash "$root/.github/scripts/install-musl-build-tools.sh"
while IFS= read -r line; do export "$line"; done < "$GITHUB_ENV"
export AWS_LC_SYS_NO_JITTER_ENTROPY=1
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_INCREMENTAL=0
cd "$root"
export CODEX_REPO_ROOT="$root"
while IFS= read -r line; do export "$line"; done < <(
  python3 - "$target" <<'PY'
import sys
from scripts.codex_package.targets import TARGET_SPECS
from scripts.codex_package.v8 import resolve_codex_v8_cargo_env
for key, value in resolve_codex_v8_cargo_env(TARGET_SPECS[sys.argv[1]]).items():
    print(f"{key}={value}")
PY
)
test -n "${RUSTY_V8_ARCHIVE:-}"
test -n "${RUSTY_V8_SRC_BINDING_PATH:-}"
export STABLE_GIT_COMMIT="$(git rev-parse HEAD)"
cd codex-rs
cargo build --locked --release --target "$target" -p codex-bwrap --bin bwrap
release="${CARGO_TARGET_DIR:-target}/$target/release"
"$strip_tool" --strip-debug --strip-unneeded "$release/bwrap"
export CODEX_BWRAP_SHA256="$(sha256sum "$release/bwrap" | cut -d ' ' -f 1)"
cargo build --locked --release --target "$target" -p codex-cli -p codex-code-mode-host --bin codex --bin codex-code-mode-host
dest="$root/dist-cocalc"
mkdir -p "$dest"
for binary in codex codex-code-mode-host bwrap; do
  if [[ "$binary" != bwrap ]]; then
    "$strip_tool" --strip-debug --strip-unneeded "$release/$binary"
  fi
  output="$dest/$binary-v0.159.2-linux-$arch"
  cp "$release/$binary" "$output"
  xz -T2 -6 -k -f "$output"
  sha256sum "$output" "$output.xz"
done
