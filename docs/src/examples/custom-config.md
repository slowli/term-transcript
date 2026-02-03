# Custom Template and Config

`term-transcript` allows overwriting the Handlebars template, or to collect all config options
into a single TOML file.

## Custom template

The `--tpl` option allows to configure a custom [Handlebars](https://handlebarsjs.com/) template
rather than the standard ones. As an example, it's possible to render a transcript [into HTML](../assets/rainbow.html).

```bash
term-transcript exec --tpl custom.html.handlebars \
  -o rainbow.html rainbow 'rainbow --short'
```

<details>
<summary><strong>Used Handlebars template</strong> (click to expand)</summary>

```handlebars+html
{{#include ../assets/custom.html.handlebars}}
```
</details>

## Configuration file

`--config-path` option allows reading rendering options from a TOML file. This enables
configuring low-level template details. The snapshot below uses a [configuration file](../assets/config.toml)
to customize palette colors and scroll animation step / interval.

![Snapshot with config read from file](../assets/custom-config.svg)

<details>
<summary><strong>Configuration file</strong> (click to expand)</summary>

```toml
{{#include ../assets/config.toml}}
```
</details>

Generating command:

```bash
term-transcript exec --config-path config.toml \
  'rainbow --long-lines'
```
