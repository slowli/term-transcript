#!/usr/bin/env bash

# Generates snapshots in this workspace.

set -e

# Extension for created snapshots. Especially important for the CLI test snapshot
# (some details are manually added to it).
EXTENSION=svg

ROOT_DIR=$(dirname "$0")
ROOT_DIR=$(realpath "$ROOT_DIR/..")
TARGET_DIR="$ROOT_DIR/target/debug"

FONT_DIR="$ROOT_DIR/examples/fonts"
FONT_ROBOTO="$FONT_DIR/RobotoMono-VariableFont_wght.ttf"
FONT_ROBOTO_ITALIC="$FONT_DIR/RobotoMono-Italic-VariableFont_wght.ttf"
FONT_FIRA="$FONT_DIR/FiraMono-Regular.ttf"
FONT_FIRA_BOLD="$FONT_DIR/FiraMono-Bold.ttf"

# Common `term-transcript` CLI args
TT_ARGS=${TT_ARGS:-"-T 250ms"}
echo "Using common args: $TT_ARGS"

(
  cd "$ROOT_DIR"
  cargo build -p term-transcript-rainbow
  cargo build -p term-transcript-cli --all-features
)

if [[ ! -x "$TARGET_DIR/term-transcript" ]]; then
  echo "Executable term-transcript not found in expected location $TARGET_DIR"
  exit 1
fi

export PATH=$TARGET_DIR:$PATH

echo "Creating rainbow snapshot..."
term-transcript exec $TT_ARGS --palette gjm8 rainbow \
  > "$ROOT_DIR/examples/rainbow.$EXTENSION"

echo "Creating rainbow snapshot (pure SVG)..."
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 rainbow \
  > "$ROOT_DIR/examples/rainbow-pure.$EXTENSION"

echo "Creating animated rainbow snapshot..."
term-transcript exec $TT_ARGS --palette powershell --scroll --pty --window \
  --line-height=18px \
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

echo "Creating wide rainbow snapshot..."
term-transcript exec $TT_ARGS --palette gjm8 \
  --hard-wrap=100 --width=900 'rainbow --long-lines' \
  > "$ROOT_DIR/examples/rainbow-wide.$EXTENSION"

echo "Creating small rainbow snapshot..."
term-transcript exec $TT_ARGS --palette gjm8 \
  --hard-wrap=50 --width=450 --scroll=180 rainbow \
  > "$ROOT_DIR/examples/rainbow-small.$EXTENSION"

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
    'ls -l Cargo.lock' 'grep -n serge Cargo.lock' 'grep -m 5 -n serde Cargo.lock' \
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
  --line-height=1.4em \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/numbers-continuous-outputs.$EXTENSION"

echo "Creating snapshot with --line-numbers continuous"
term-transcript exec $TT_ARGS --palette gjm8 --line-numbers continuous \
  --scroll --scroll-interval 2s --scroll-len 2em \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/numbers-continuous.$EXTENSION"

echo "Creating snapshot with --line-numbers continuous (pure SVG)"
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 --line-numbers continuous \
  --scroll --scroll-interval 2s --scroll-len 2em \
  rainbow 'rainbow --short' \
  > "$ROOT_DIR/examples/numbers-continuous-pure.$EXTENSION"

echo "Creating snapshot with --line-numbers and long lines"
term-transcript exec $TT_ARGS --palette gjm8 --line-numbers continuous \
  --line-height=18px \
  'rainbow --long-lines' \
  > "$ROOT_DIR/examples/numbers-long.$EXTENSION"

echo "Creating snapshot with --line-numbers and long lines (pure SVG)"
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 --line-numbers continuous \
  --line-height=18px --advance-width=7.8px \
  'rainbow --long-lines' \
  > "$ROOT_DIR/examples/numbers-long-pure.$EXTENSION"

# Backup fonts are for the case if CSP prevents CSS / font loading from the CDN
echo "Creating snapshot with Fira Mono font..."
term-transcript exec $TT_ARGS --palette gjm8 --window \
  --font 'Fira Mono, Consolas, Liberation Mono, Menlo' \
  --styles '@import url(https://code.cdn.mozilla.net/fonts/fira.css);' rainbow \
  > "$ROOT_DIR/examples/fira.$EXTENSION"
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 --window \
  --font 'Fira Mono, Consolas, Liberation Mono, Menlo' \
  --styles '@import url(https://code.cdn.mozilla.net/fonts/fira.css);' rainbow \
  > "$ROOT_DIR/examples/fira-pure.$EXTENSION"

echo "Creating snapshot with custom config..."
term-transcript exec $TT_ARGS --config-path "$ROOT_DIR/examples/config.toml" \
  'rainbow --long-lines' \
  > "$ROOT_DIR/examples/custom-config.$EXTENSION"

echo "Creating snapshot with --embed-font (Roboto Mono, var weight)"
term-transcript exec $TT_ARGS --palette gjm8 --line-numbers continuous \
  --embed-font="$FONT_ROBOTO" \
  'rainbow --short' \
  > "$ROOT_DIR/examples/embedded-font.$EXTENSION"

echo "Creating snapshot with --embed-font (Roboto Mono, var weight + italic), --pure-svg"
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 --line-numbers continuous \
  --line-height=1.4em \
  --embed-font="$FONT_ROBOTO:$FONT_ROBOTO_ITALIC" \
  'rainbow --short' \
  > "$ROOT_DIR/examples/embedded-font-pure.$EXTENSION"

echo "Creating snapshot with --embed-font (Fira Mono, regular + bold), --pure-svg"
term-transcript exec $TT_ARGS --pure-svg --palette gjm8 --line-numbers continuous \
  --embed-font="$FONT_FIRA:$FONT_FIRA_BOLD" \
  --advance-width=8.6px \
  'rainbow --short' \
  > "$ROOT_DIR/examples/embedded-font-fira.$EXTENSION"
