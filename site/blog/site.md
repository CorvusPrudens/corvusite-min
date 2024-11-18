---
title: Designing a minimal website
date: 11/2/24
description: |
  It might involve more than you think!
---

# Designing a minimal website

Ever since I read [this article about the internet in the Antarctic](https://brr.fyi/posts/engineering-for-slow-internet),
I've thought a lot about how my designs may affect those with
underpowered devices or slow connections.
While it's not very hard to make a performant blog site,
I wanted to go as minimal as possible while still
maintaining a nice authoring experience.

Naturally, that meant I had to write a bunch of tools for myself! [^1]

[^1]:
    There are, of course, many frameworks that can
    do [SSG](https://nextjs.org/docs/pages/building-your-application/rendering/static-site-generation).
    However, I can't stand the JavaScript ecosystem,
    so I wasn't interested in taking that approach.

## Components

Components are nice.[^2] They behave much like functions, which are also nice.

[^2]:
    [Web components](https://developer.mozilla.org/en-US/docs/Web/API/Web_components) are
    not that nice in my opinion, so I didn't make use of them.

Since I knew I wouldn't need much interactivity, I settled on a very simple approach:
a component is just a custom element. It's defined in its own file and
can declare properties. It can also contain a `children` element,
describing where its children should go when expanded.

```html
<!-- button-link.mod.html -->
<ButtonLink href aria-label>
  <a class="button-link" href="href" aria-label="aria-label">
    <children />
  </a>
</ButtonLink>
```

At the usage site, the component is recursively expanded.

```html
<!-- my-page.html -->
<!-- Usage -->
<div>
  <ButtonLink href="/hello-world" aria-label="A component example">
    Hello, world!
  </ButtonLink>
</div>

<!-- Expanded -->
<div>
  <a class="button-link" href="/hell-world" aria-label="A component example">
    Hello, world!
  </a>
</div>
```

While simple, this approach has been good enough to build this site so far
without any trouble.

## Markdown

For articles, I set up a markdown processing pipeline.
There are a number of decent markdown parsing libraries
on [crates.io](https://crates.io), including [markdown](https://crates.io/crates/markdown)
and [comrak](https://crates.io/crates/comrak), but I preferred
to write my own.

Rolling my own presented a few advantages. For one, I only
needed to support the subset of markdown I'm actually using.
More importantly, I could write a _nearly_ allocation-free algorithm.
This was easy enough to achieve by applying the visitor pattern,
rather than building an [abstract syntax tree](https://en.wikipedia.org/wiki/Abstract_syntax_tree)
like `comrak` and `markdown`.

On a fairly representative 12kB markdown file, the visitor approach was
around 9 times faster than `comrak`
and 71 times faster than `markdown`.
(This isn't totally free, though. A pure, single-pass visitor
cannot be fully [CommonMark](https://spec.commonmark.org/) compliant.
As an example, link definitions
can occur arbitrarily far after link references.)

Does this performance actually matter? Well, not really.
This is a pre-processing step, so the client won't be parsing any markdown either way.
Furthermore, for this site's files, we're talking about the difference between
10 microseconds and 100 microseconds per markdown file.
The syntax highlighting library I'm using is significantly slower,
so that ends up being the real bottleneck anyway.

But it feels nice to write something _fast_, doesn't it?

## Eager fetching and rendering

The [Speculation Rules API](https://developer.mozilla.org/en-US/docs/Web/API/Speculation_Rules_API)
provides a means to easily prefetch and pre-render webpages.
It's only available in more recent Chromium-based browsers (typical),
but it's a great tool for making navigation feel fast.
And since the browser handles all the speculation, it's
super easy to use.

For this site, the rules look like:

```html
<script type="speculationrules">
  {
    "prefetch": [
      {
        "where": { "selector_matches": ".prerender-link" },
        "eagerness": "eager"
      }
    ],
    "prerender": [
      {
        "where": { "selector_matches": ".prerender-link" },
        "eagerness": "eager"
      }
    ]
  }
</script>
```

That is, the browser will eagerly fetch and render any link
that matches the `.prerender-link` selector.

## Wrapping up

Combined with hosting on GitHub Pages
(which is [surprisingly fast](https://simplyexplained.com/blog/benchmarking-static-website-hosting-providers/)),
these all come together to make a decently fast site for the user.
And since this project's complexity is low, it was
easy to streamline the development experience as well.
Whether I'm writing an article in markdown or modifying
the site's HTML or CSS, the whole thing can be processed and
reloaded in a few milliseconds.

Computers really can fly if you let them.
