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

[Inkscape]: https://inkscape.org/
