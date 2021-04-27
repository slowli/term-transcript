#!/usr/bin/env bash

# Generates snapshots in this workspace.

set -e

# Extension for created snapshots. Especially important for the CLI test snapshot
# (some details are manually added to it).
EXTENSION=new.svg

ROOT_DIR=$(dirname "$0")
ROOT_DIR=$(realpath -L "$ROOT_DIR/..")
TARGET_DIR="$ROOT_DIR/target/debug"

(cd "$ROOT_DIR"; cargo build -p term-transcript-cli -p term-transcript-rainbow)

if [[ ! -x "$TARGET_DIR/term-transcript" ]]; then
  echo "Executable term-transcript not found in expected location $TARGET_DIR"
  exit 1
fi

export PATH=$PATH:$TARGET_DIR

echo "Creating rainbow snapshot..."
 term-transcript exec -T 100 --palette gjm8 rainbow \
  > "$ROOT_DIR/examples/rainbow.$EXTENSION"

echo "Creating aliased rainbow snapshot..."
rainbow | term-transcript capture 'colored-output' \
  > "$ROOT_DIR/e2e-tests/rainbow/aliased.$EXTENSION"

echo "Creating CLI test snapshot..."
export COLOR=always
term-transcript exec -T 500 --palette xterm --window \
  'term-transcript exec -T 100 rainbow > /tmp/rainbow.svg' \
  'term-transcript test -T 100 -v /tmp/rainbow.svg' \
  > "$ROOT_DIR/cli/tests/snapshots/test.$EXTENSION"
