#!/usr/bin/env bash

# Script to create an archive with the release contents (the `term-transcript` executable
# and the supporting docs).

set -e

VERSION=$1
if [[ "$VERSION" == '' ]]; then
  echo "Error: release version is not specified"
  exit 1
fi
echo "Packaging term-transcript $VERSION for $TARGET..."

CLI_DIR=$(dirname "$0")
RELEASE_DIR="$CLI_DIR/release"
ROOT_DIR="$CLI_DIR/../.."
EXECUTABLE="$ROOT_DIR/target/$TARGET/executable/term-transcript"

if [[ "$OS" == 'windows-latest' ]]; then
  EXECUTABLE="$EXECUTABLE.exe"
fi
if [[ ! -x $EXECUTABLE ]]; then
  echo "Error: executable $EXECUTABLE does not exist"
  exit 1
fi

rm -rf "$RELEASE_DIR" && mkdir "$RELEASE_DIR"
echo "Copying release files to $RELEASE_DIR..."
cp "$EXECUTABLE" \
  "$CLI_DIR/README.md" \
  "$CLI_DIR/CHANGELOG.md" \
  "$CLI_DIR/LICENSE-APACHE" \
  "$CLI_DIR/LICENSE-MIT" \
  "$RELEASE_DIR"

cd "$RELEASE_DIR"
echo "Creating release archive..."
case $OS in
  ubuntu-latest | macos-latest)
    ARCHIVE="term-transcript-$VERSION-$TARGET.tar.gz"
    tar czf "$ARCHIVE" ./*
    ;;
  windows-latest)
    ARCHIVE="term-transcript-$VERSION-$TARGET.zip"
    7z a "$ARCHIVE" ./*
    ;;
  *)
    echo "Unknown target: $TARGET"
    exit 1
esac
ls -l "$ARCHIVE"

if [[ "$GITHUB_OUTPUT" != '' ]]; then
  echo "Outputting path to archive as GitHub step output: $RELEASE_DIR/$ARCHIVE"
  echo "archive=$RELEASE_DIR/$ARCHIVE" >> "$GITHUB_OUTPUT"
fi
