# Controlling Inputs

`--no-inputs` flag allows hiding user inputs in the generated snapshots.

## Hiding all inputs

![Hidden user inputs](../assets/no-inputs-numbers.svg)

Generating command:

```bash
term-transcript exec --scroll --palette xterm \
  --no-inputs --line-numbers continuous \
  rainbow 'rainbow --short'
```

Same snapshot generated using the pure SVG template (i.e., with the additional
`--pure-svg` flag):

![Hidden user inputs, pure SVG](../assets/no-inputs-numbers-pure.svg)

```bash
term-transcript exec --pure-svg --scroll --palette xterm \
  --no-inputs --line-numbers continuous \
  rainbow 'rainbow --short'
```
