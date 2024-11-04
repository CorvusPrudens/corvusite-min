---
title: Designing a minimal website
date: 11/2/24
---

# Designing a minimal website

Like many programmers, I'm attracted to optimization.

## Components

Components are nice. They provide convenient encapsulation and
a familiar, modern workflow.

For this project, I settled on a very simple approach.
I would define a component in a special file with an
explicitly declared set of attributes and an optional
`children` element.

```html
<!-- button-link.mod.html -->
<ButtonLink href aria-label>
  <a
    class="button-link rounded"
    href="href"
    aria-label="aria-label"
    title="aria-label"
  >
    <children />
  </a>
</ButtonLink>
```

At the usage site, the component node is recursively expanded.

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
  <a
    class="button-link rounded"
    href="/hell-world"
    aria-label="A component example"
    title="A component example"
  >
    Hello, world!
  </a>
</div>
```

This approach is very limited, but it's ergonomic
enough for a small site with limited interactivity.

## Markdown

For articles, I set up a markdown processing pipeline.
There are a number of decent markdown parsing libraries
on [crates.io](https://crates.io), including [markdown](https://crates.io/crates/markdown)
and [comrak](https://crates.io/crates/comrak), but I was interested
in writing my own.

Rolling my own presented a few advantages. For one, I only
needed to support the subset of markdown I'm actually using.
More importantly, I could write a _nearly_ allocation-free algorithm.
This was easy enough to achieve by applying the visitor pattern,
rather than building an [abstract syntax tree](https://en.wikipedia.org/wiki/Abstract_syntax_tree)
like `comrak` and `markdown`.

To illustrate the difference, here's how you might
generate HTML using an AST.

```rs
// AST
enum Node<'s> {
  // Even if we write a zero-copy parser,
  // we'll need allocations for nodes.
  Paragraph(Vec<Node<'s>>),
  InlineCode(&'s str),
  // and so on...
}

fn process_node(node: &Node<'_>, buffer: &mut Vec<u8>) {
  match node {
    Node::Paragraph(children) => {
      write!(buffer, "<p>").unwrap();
      for child in children {
        process_node(child, buffer);
      }
      write!(buffer, "</p>").unwrap();
    }
    Node::InlineCode(code) => {
      write!(buffer, "<code>{code}</code>").unwrap();
    }
    // ...
  }
}

fn process_with_ast(input: &str) -> Vec<u8> {
  // Processing happens in two steps:
  // First, we build the AST.
  let tree = Node::parse(input);

  // Then, we walk the tree and produce the output.
  let mut output = Vec::new();
  process_node(&tree, &mut output);

  output
}
```

And here's how you might do the same with a visitor.

```rs
trait Visitor {
  // For nodes that could have an arbitrary number
  // of children, we need enter and exit methods.
  fn paragraph_enter(&mut self);
  fn paragraph_exit(&mut self);

  // Simpler nodes can be processed in one go.
  fn inline_code(&mut self, code: &str);

  // and so on...
}

struct VecVisitor(Vec<u8>);

impl Visitor for VecVisitor {
  fn paragraph_enter(&mut self) {
    write!(&mut self.0, "<p>").unwrap();
  }

  fn paragraph_exit(&mut self) {
    write!(&mut self.0, "</p>").unwrap();
  }

  fn inline_code(&mut self, code: &str) {
    write!(&mut self.0, "<code>{code}</code>").unwrap();
  }

  // ...
}

fn process_with_visitor(input: &str) -> Vec<u8> {
  let mut visitor = VecVisitor(Vec::new());

  // This time, processing happens in a single step
  // and requires no intermediate allocations.
  parse(input, &mut visitor);

  visitor.0
}
```

On a fairly representative 12kB markdown file, the visitor approach was
around 12 times faster than `comrak`
and 117 times faster than `markdown`. I figured that processing
all the markdown files for this site within a handful of microseconds
was probably fast enough, so I didn't go digging for more optimizations.

## Eager fetching and rendering

The [Speculation Rules API](https://developer.mozilla.org/en-US/docs/Web/API/Speculation_Rules_API)
provides a means to easily prefetch and pre-render webpages.
