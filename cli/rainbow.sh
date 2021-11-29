#!/usr/bin/env sh

# Standalone shell script to output various text styles

BASE_COLORS="black red green yellow blue magenta cyan white"
RGB_COLOR_NAMES="pink orange brown teal"
RGB_COLOR_VALUES="255;187;221 255;170;68 159;64;16 16;136;159"

index() {
  shift "$1"
  echo "$2"
}

base_colors_line() {
  start_code="$1"
  underline_oddity="$2"

  line=""
  for i in $(seq 0 7); do
    color=$((i + start_code))
    decor=""
    if [ $((i % 2)) -eq "$underline_oddity" ]; then
      decor='\e[4m' # underline
    fi
    line=$line'\e['$color'm'$decor$(index "$i" $BASE_COLORS)'\e[0m '
  done
  echo "$line"
}

ansi_colors_line() {
  line=""
  for i in $(seq 16 231); do
    fg_color="\e[37m" # white
    col=$(((i - 16) % 36))
    if [ "$col" -gt 18 ]; then
      fg_color="\e[30m" # black
    fi
    line=$line'\e[38;5;'$i'm!\e[0m'$fg_color'\e[48;5;'$i'm?\e[0m'

    if [ "$col" -eq 35 ]; then
      echo "$line"
      line=""
    fi
  done
}

ansi_grayscale_line() {
  line=""
  for i in $(seq 232 255); do
    fg_color="\e[37m" # white
    if [ "$i" -ge 244 ]; then
      fg_color="\e[30m" # black
    fi
    line=$line'\e[38;5;'$i'm!\e[0m'$fg_color'\e[48;5;'$i'm?\e[0m'
  done
  echo "$line"
}

rgb_colors_line() {
  line=""
  for i in $(seq 0 3); do
    name=$(index "$i" $RGB_COLOR_NAMES)
    value=$(index "$i" $RGB_COLOR_VALUES)
    line=$line'\e[38;2;'$value'm'$name'\e[0m '
  done
  echo "$line"
}

echo "Base colors:"
base_colors_line 30 0
base_colors_line 90 1
echo "Base colors (bg):"
base_colors_line 40 2
base_colors_line 100 2

if [ "$1" = "--short" ]; then
  exit 0
fi

echo "ANSI color palette:"
ansi_colors_line
echo "ANSI grayscale palette:"
ansi_grayscale_line

echo "24-bit colors:"
rgb_colors_line
