#!/usr/bin/env bash
# Compares the Harness Memory, Code and Knowledge binaries in ~/.local/bin with their latest release (by SHA-256, so a
# custom build is found even without a -dev suffix) and restores the named ones with their official installer.
#   no arguments        report only, changes nothing
#   memory code ...     restore those members
#   all                 restore every member that differs
# Exit: 0 fine, 1 a check or restore failed, 2 bad arguments.
set -euo pipefail

OWNER="MSpiechowicz"
BIN_DIR="$HOME/.local/bin"
MEMBERS="memory code knowledge"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64 | Linux-amd64) TRIPLE="x86_64-unknown-linux-gnu" ;;
  Darwin-arm64 | Darwin-aarch64) TRIPLE="aarch64-apple-darwin" ;;
  Darwin-x86_64) TRIPLE="x86_64-apple-darwin" ;;
  *) echo "unsupported platform: $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

RESTORE=""
for arg in "$@"; do
  case "$arg" in
    all) RESTORE="$MEMBERS" ;;
    memory | code | knowledge) RESTORE="$RESTORE $arg" ;;
    *) echo "unknown argument: $arg (use memory, code, knowledge or all)" >&2; exit 2 ;;
  esac
done

TMP="$(mktemp -d)"
cleanup() { rm -f "$TMP"/*; rmdir "$TMP"; }
trap cleanup EXIT

sha() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'; else shasum -a 256 "$1" | awk '{print $1}'; fi
}

fetch() { curl -fsSL --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 "$@"; }

wanted() { case " $RESTORE " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

reports() {
  if [ -x "$1" ]; then "$1" --version 2>/dev/null | head -1 | awk '{print $NF}' || true; else echo "missing"; fi
}

rc=0
changed=0

for member in $MEMBERS; do
  repo="harness-useful-$member"
  bin="harness-$member"
  path="$BIN_DIR/$bin"
  upper="$(printf '%s' "$member" | tr '[:lower:]' '[:upper:]')"

  url="$(fetch -o /dev/null -w '%{url_effective}' "https://github.com/$OWNER/$repo/releases/latest")" || url=""
  tag="${url##*/}"
  if ! printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "$bin: FAILED, no latest release found"; rc=1; continue
  fi

  if ! fetch -o "$TMP/$member.sums" "https://github.com/$OWNER/$repo/releases/download/$tag/checksums.txt"; then
    echo "$bin: FAILED, could not read the checksums of $tag"; rc=1; continue
  fi
  asset="$bin-$TRIPLE"
  expected="$(awk -v name="$asset" 'substr($0, length($1) + 3) == name { print $1; exit }' "$TMP/$member.sums")"
  if [ -z "$expected" ]; then
    echo "$bin: FAILED, $tag lists no checksum for $asset"; rc=1; continue
  fi

  actual=""
  [ -f "$path" ] && actual="$(sha "$path")"
  now="$(reports "$path")"

  if [ "$actual" = "$expected" ]; then
    echo "$bin: stable, $now is release $tag"
    continue
  fi

  if ! wanted "$member"; then
    echo "$bin: DIFFERS, reports ${now:-no version}, latest release is $tag (not restored)"
    continue
  fi

  if ! fetch -o "$TMP/$member-install.sh" "https://raw.githubusercontent.com/$OWNER/$repo/main/install.sh"; then
    echo "$bin: FAILED, could not download its installer"; rc=1; continue
  fi
  if ! env "HARNESS_${upper}_VERSION=$tag" "HARNESS_${upper}_NO_MODIFY_PATH=1" sh "$TMP/$member-install.sh" >"$TMP/$member.log" 2>&1; then
    cat "$TMP/$member.log"
    echo "$bin: FAILED, the installer stopped, the binary was left as it was"; rc=1; continue
  fi

  if [ "$(sha "$path")" = "$expected" ]; then
    echo "$bin: RESTORED, ${now:-no version} -> $(reports "$path") (release $tag)"
    changed=1
  else
    echo "$bin: FAILED, the installed file does not match the $tag checksum"; rc=1
  fi
done

if [ "$changed" = 1 ]; then
  echo "Restart Harness Desktop and any open agent session: they keep the old binary until their server starts again."
fi
exit "$rc"
