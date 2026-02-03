# Configuring Window Appearance

There are dedicated args to control window sizing and title.

## Width

![Wide snapshot](../assets/rainbow-wide.svg)

Use `--width` to control the pixel width of the console, and `--hard-wrap` to control
at which char the console output is hard-wrapped to a new line. It usually makes sense
to set these both params: `width ≈ hard_wrap * 9` (the exact coefficient depends on
the font being used).

Generating command:

```bash
term-transcript exec --palette gjm8 \
  --hard-wrap=100 --width=900 'rainbow --long-lines'
```

## Scroll height

![Small snapshot](../assets/rainbow-small.svg)

Use `--scroll=$height` to set the maximum pixel height of the snapshot.

Generating command:

```bash
term-transcript exec --palette gjm8 \
  --hard-wrap=50 --width=450 --scroll=180 rainbow
```

## Scroll animation

Besides the scroll height, the following command-line args control the scroll animation:

- `--scroll-interval` (e.g., `2s`): Configures the interval between animation frames.
- `--scroll-len` (e.g., `3em`): Height scrolled during each animation frame (other than possibly the last frame).

See [line numbering snapshots](line-numbering.md) for examples.

## Window frame and title

`--window` arg allows to add a macOS-like window frame to the snapshot.
The same arg can be used to set the window title. (If not specified, the title will be empty.)

See [an animated snapshot](basics.md#animated-snapshot) for an example.
