#!/usr/bin/env bash

# Generates snapshots in this workspace.

set -e

# Extension for created snapshots. Especially important for the CLI test snapshot
# (some details are manually added to it).
EXTENSION=new.svg

ROOT_DIR=$(dirname "$0")
ROOT_DIR=$(realpath -L "$ROOT_DIR/..")
TARGET_DIR="$ROOT_DIR/target/debug"
CLI_TARGET_DIR="$ROOT_DIR/cli/target/debug"

# Common `term-transcript` CLI args
TT_ARGS="-T 250ms"

(
  cd "$ROOT_DIR"
  cargo build -p term-transcript-rainbow
  cargo build --manifest-path=cli/Cargo.toml --all-features
)

if [[ ! -x "$CLI_TARGET_DIR/term-transcript" ]]; then
  echo "Executable term-transcript not found in expected location $CLI_TARGET_DIR"
  exit 1
fi

export PATH=$PATH:$TARGET_DIR:$CLI_TARGET_DIR

echo "Creating rainbow snapshot..."
term-transcript exec $TT_ARGS --palette gjm8 rainbow \
  > "$ROOT_DIR/examples/rainbow.$EXTENSION"

echo "Creating rainbow snapshot (pure SVG)..."
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 rainbow \
  > "$ROOT_DIR/examples/rainbow-pure.$EXTENSION"

echo "Creating animated rainbow snapshot..."
term-transcript exec $TT_ARGS --palette powershell --pty --window --scroll \
  rainbow 'rainbow --long-lines' \
  > "$ROOT_DIR/examples/animated.$EXTENSION"

echo "Creating aliased rainbow snapshot..."
rainbow | term-transcript capture 'colored-output' \
  > "$ROOT_DIR/e2e-tests/rainbow/aliased.$EXTENSION"

echo "Creating REPL snapshot..."
term-transcript exec $TT_ARGS --shell rainbow-repl \
  'yellow intense bold green cucumber' \
  'neutral #fa4 underline #c0ffee' \
  '#9f4010 (brown) italic' \
  > "$ROOT_DIR/e2e-tests/rainbow/repl.$EXTENSION"

echo "Creating snapshot with custom template..."
term-transcript exec $TT_ARGS --palette xterm \
  --tpl "$ROOT_DIR/examples/custom.html.handlebars" \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/rainbow.new.html"

echo "Creating snapshot with failure..."
term-transcript exec $TT_ARGS --palette gjm8 --window \
  './non-existing-command' '[ -x non-existing-file ]' '[ -x non-existing-file ] || echo "File is not there!"' \
  > "$ROOT_DIR/examples/failure-sh.$EXTENSION"

echo "Creating PTY snapshot with failure..."
(
  cd "$ROOT_DIR"
  term-transcript exec $TT_ARGS --palette gjm8 --pty --window --shell bash \
    'ls -l Cargo.lock' 'grep -n serge Cargo.lock' 'grep -n serde Cargo.lock' \
    > "$ROOT_DIR/examples/failure-bash-pty.$EXTENSION"
)

echo "Creating snapshot with --line-numbers each-output"
term-transcript exec $TT_ARGS --scroll --palette xterm --line-numbers each-output \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/numbers-each-output.$EXTENSION"

echo "Creating snapshot with no inputs, --line-numbers continuous"
term-transcript exec $TT_ARGS --scroll --palette xterm \
  --no-inputs --line-numbers continuous \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/no-inputs-numbers.$EXTENSION"

echo "Creating snapshot with no inputs, --line-numbers continuous (pure SVG)"
term-transcript exec $TT_ARGS --scroll --palette xterm --pure-svg \
  --no-inputs --line-numbers continuous \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/no-inputs-numbers-pure.$EXTENSION"

echo "Creating snapshot with --line-numbers continuous-outputs"
term-transcript exec $TT_ARGS --scroll --palette powershell --line-numbers continuous-outputs \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/numbers-continuous-outputs.$EXTENSION"

echo "Creating snapshot with --line-numbers continuous"
term-transcript exec $TT_ARGS --scroll --palette gjm8 --line-numbers continuous \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/numbers-continuous.$EXTENSION"

echo "Creating snapshot with --line-numbers continuous (pure SVG)"
term-transcript exec $TT_ARGS --pure-svg --scroll --palette gjm8 --line-numbers continuous \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/numbers-continuous-pure.$EXTENSION"

echo "Creating snapshot with --line-numbers and long lines"
term-transcript exec $TT_ARGS --palette gjm8 --line-numbers continuous \
  'rainbow --long-lines' \
  > "$ROOT_DIR/examples/numbers-long.$EXTENSION"

echo "Creating snapshot with --line-numbers and long lines (pure SVG)"
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 --line-numbers continuous \
  'rainbow --long-lines' \
  > "$ROOT_DIR/examples/numbers-long-pure.$EXTENSION"

# Backup fonts are for the case if CSP prevents CSS / font loading from the CDN
echo "Creating snapshot with Fira Mono font..."
term-transcript exec $TT_ARGS --palette gjm8 --window \
  --font 'Fira Mono, Consolas, Liberation Mono, Menlo' \
  --styles '@import url(https://code.cdn.mozilla.net/fonts/fira.css);' rainbow \
  > "$ROOT_DIR/examples/fira.$EXTENSION"
