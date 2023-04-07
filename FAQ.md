# Frequently Asked Questions

This file provides some tips and troubleshooting advice for `term-transcript`
in the FAQ format.

## HTML embedding not supported error

If the generated SVG file contains a single red line of text "HTML embedding not supported...",
it means that you view it using a program that does not support HTML embedding for SVG.
That is, the real transcript is still there, it is just not rendered properly by a particular viewer.
All modern web browsers support HTML embedding (since they support HTML rendering anyway),
but some other SVG viewers, such as [Inkscape], don't.

To give more technical background why HTML embedding is used in the first place:
SVG isn't good at text layout, particularly for multiline text and/or text with background coloring.
HTML, on the other hand, can lay out such text effortlessly. Thus, `term-transcript`
avoids the need of layout logic by embedding pieces of HTML (essentially, `<pre>`-formatted `<span>`s)
into the generated SVGs.

## Transcripts & Content Security Policy

A potential reason for rendering errors when the transcript SVG is viewed from a browser
is [Content Security Policy (CSP)][CSP] set by the HTTP server.
If this is the case, the developer console will contain
an error mentioning the policy, e.g. "Refused to apply inline style because it violates
the following Content Security Policy...". To properly render a transcript, the CSP should contain
the `style-src 'unsafe-inline'` permission.

As an example, GitHub does not provide sufficient CSP permissions for the files attached to issues,
comments, etc. On the other hand, *committed* files are served with adequate permissions;
they can be linked to using an URL like `https://github.com/$user/$repo/raw/HEAD/path/to/snapshot.svg?sanitize=true`.

## Customizing fonts

It is possible to customize the font used in the transcript using `font_family` and `additional_styles`
fields in [`TemplateOptions`] (when using the Rust library), or `--font` / `--styles` arguments
(when using the CLI app).

For example, the [Fira Mono](https://github.com/mozilla/Fira) font family can be included
by setting additional styles to the following value:

 ```css
@import url(https://code.cdn.mozilla.net/fonts/fira.css);
```

It is possible to include `@font-face`s directly instead, which can theoretically
be used to embed the font family via [data URLs]:

```css
@font-face {
  font-family: 'Fira Mono';
  src: local('Fira Mono'), url('data:font/woff;base64,...') format('woff');
  /* base64-encoded WOFF font snipped above */
  font-weight: 400;
  font-style: normal;
}
```

Such embedding, however, typically leads to a huge file size overhead (hundreds of kilobytes)
unless the fonts are subsetted beforehand (minimized to contain only glyphs necessary
to render the transcript).

Beware that if a font is included from an external source, it may be subject to CSP restrictions
as described [above](#transcripts--content-security-policy).

[Inkscape]: https://inkscape.org/
[CSP]: https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP
[`TemplateOptions`]: https://slowli.github.io/term-transcript/term_transcript/svg/struct.TemplateOptions.html
[data URLs]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URLs
