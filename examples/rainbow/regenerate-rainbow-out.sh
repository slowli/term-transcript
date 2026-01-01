#!/usr/bin/env sh

# Regenerates outputs of the `rainbow` script so that they can be used by `rainbow.bat`.

set -e

RAINBOW_DIR="$(dirname $0)"
RAINBOW_SCRIPT="$RAINBOW_DIR/rainbow"
echo "Regenerating rainbow outputs using $RAINBOW_SCRIPT"

"$RAINBOW_SCRIPT" > "$RAINBOW_DIR/rainbow.out"
"$RAINBOW_SCRIPT" --long-lines > "$RAINBOW_DIR/rainbow-long.out"
"$RAINBOW_SCRIPT" --short > "$RAINBOW_DIR/rainbow-short.out"
