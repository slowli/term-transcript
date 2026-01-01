#!/usr/bin/env sh

# Regenerates outputs of the `rainbow` script so that they can be used by `rainbow.bat`.

set -e

RAINBOW_DIR="$(dirname $0)"
RAINBOW_SCRIPT="$RAINBOW_DIR/rainbow"

CHECK_MODE=
if [ "$1" = "--check" ]; then
  CHECK_MODE=1
fi

run_script() {
  args="$1"
  out_file="$RAINBOW_DIR/$2.out"

  if [ "$CHECK_MODE" ]; then
    echo "Checking rainbow output for $RAINBOW_SCRIPT $args from $out_file"
    "$RAINBOW_SCRIPT" $args | diff - "$out_file"
  else
    echo "Regenerating rainbow output using $RAINBOW_SCRIPT $args to $out_file"
    "$RAINBOW_SCRIPT" $args > "$out_file"
  fi
}

run_script '' 'rainbow'
run_script '--long-lines' 'rainbow-long'
run_script '--short' 'rainbow-short'
