# Basic Usage

## Static snapshot

![Snapshot of rainbow example](../assets/rainbow.svg)

Generating command:

```bash
term-transcript exec --palette gjm8 rainbow
```

(`rainbow` is an executable script for [end-to-end tests](rainbow/rainbow).)

## Static snapshot (pure SVG)

![Snapshot of rainbow example](../assets/rainbow-pure.svg)

Generating command:

```bash
term-transcript exec --pure-svg --palette gjm8 rainbow
```

## Animated snapshot

![Animated snapshot of rainbow example](../assets/animated.svg)

Generating command:

```bash
term-transcript exec --palette powershell --line-height=18px \
   --scroll --pty --window='rainbow, rainbow --long-lines' \
   rainbow 'rainbow --long-lines'
```

Note the `--pty` flag to use a pseudo-terminal for capture instead of default pipes,
and an increased line height.
