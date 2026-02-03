# Failed Inputs

Some shells may allow detecting whether an input resulted in a failure
(e.g., *nix shells allow doing this by comparing the output of `echo $?` to 0,
while in PowerShell `$?` can be compared to `True`). Such failures are captured
and visually highlighted the default SVG template.

## Failures in `sh`

![Snapshot with failing `sh` commands](../assets/failure-sh.svg)

Generating command:

```bash
term-transcript exec --palette gjm8 --window='sh Failures' --shell sh \
  'which non-existing-command > /dev/null' \
  '[ -x non-existing-file ]' \
  '[ -x non-existing-file ] || echo "File is not there!"'
```

## Failures in `bash`

Captured using a pseudo-terminal, hence colorful `grep` output.

![Snapshot with failing `grep` in `bash`](../assets/failure-bash-pty.svg)

Generating command:

```bash
term-transcript exec --palette gjm8 \
  --pty --window='bash Failures' --shell bash \
  --init 'export PS1=' \
  --init 'export GREP_COLORS="mt=01;31:ln=:se="' \
  --init 'alias grep="grep --color=always"' \
  'grep -n orange config.toml' \
  'grep -m 5 -n blue config.toml'
```

Setting `GREP_COLORS` on Linux emulates `grep` coloring on macOS / *BSD (only supports coloring matched text,
not line numbers and separators).

## Failures in `pwsh`

![Snapshot with failing `pwsh` command](../assets/failure-pwsh.svg)

```bash
term-transcript exec --window --palette gjm8 \
  --shell pwsh './non-existing-command'
```
