pub mod arena;
pub mod mdast;
pub mod visitor;

// use anyhow::Context;
// use convert_case::{Case, Casing};
// use markdown::{
//     mdast::{
//         Blockquote, Code, Definition, Delete, Emphasis, Heading, Html, Image, InlineCode,
//         InlineMath, Link, List, ListItem, Math, Node, Paragraph, Root, Strong, Text, Yaml,
//     },
//     Constructs,
// };
// use serde::{Deserialize, Serialize};
//
// #[derive(Debug, Serialize, Deserialize)]
// pub struct Blog {
//     pub title: String,
//     pub description: Option<String>,
//     pub date: String,
//     pub image_path: Option<String>,
// }
//
// impl Blog {
//     fn slug(&self) -> String {
//         slug::slugify(&self.title)
//     }
//
//     fn identifier(&self) -> proc_macro2::Ident {
//         quote::format_ident!("{}", self.title.to_case(Case::Pascal))
//     }
// }
//
// #[derive(Debug, Serialize, Deserialize)]
// pub struct Component {
//     pub signature: String,
// }
//
// #[derive(Debug, Serialize, Deserialize)]
// #[serde(untagged)]
// pub enum Frontmatter {
//     Blog(Blog),
//     Component(Component),
// }
//
// /// Represents the final, parsed markdown
// #[derive(Debug)]
// pub struct ParsedMarkdown {
//     pub frontmatter: Frontmatter,
//     /// Expressions intended to run as actual code, placed
//     /// in sequential order before the view! macro
//     pub expressions: Vec<String>,
//     /// The markdown parsed down to leptos view! syntax
//     pub body: String,
//     pub include_katex_css: bool,
// }
//
// impl ParsedMarkdown {
//     pub fn from_markdown(input: &str) -> anyhow::Result<Self> {
//         let mut constructs = Constructs::gfm();
//         constructs.math_flow = true;
//         constructs.math_text = true;
//         constructs.frontmatter = true;
//
//         let options = markdown::ParseOptions {
//             math_text_single_dollar: true,
//             constructs,
//             ..Default::default()
//         };
//         let tree = markdown::to_mdast(input, &options).map_err(anyhow::Error::msg)?;
//
//         // panic!("{tree:#?}");
//         let mut partial = PartialParse::new();
//
//         let body = walk_tree(&tree, &mut partial)?;
//
//         partial.into_parsed(body)
//     }
//
//     fn to_component(&self) -> TokenStream {
//         match &self.frontmatter {
//             Frontmatter::Blog(
//                 blog @ Blog {
//                     title,
//                     description,
//                     date,
//                     image_path,
//                 },
//             ) => {
//                 let ident = blog.identifier();
//
//                 let expressions = self.expressions.join("\n");
//                 let expressions: proc_macro2::TokenStream = expressions
//                     .parse()
//                     .expect("Expressions should be well-formed");
//
//                 let description = match description {
//                     Some(d) => quote! { Some(#d) },
//                     None => quote! { None },
//                 };
//
//                 let body: proc_macro2::TokenStream = self
//                     .body
//                     .parse()
//                     .expect("Generated blog body should be well-formed");
//
//                 // TODO -- set up image macro
//                 let katex_css = self.include_katex_css.then(|| quote! {
//                     <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.css" />
//                 });
//
//                 quote! {
//                     // use leptos::prelude::*;
//
//                     // pub const TITLE: &str = #title;
//                     // pub const DESCRIPTION: Option<&str> = #description;
//                     // pub const DATE: &str = #date;
//
//                     #[component]
//                     fn #ident() -> impl IntoView {
//                         #expressions
//
//                         view! {
//                             #katex_css
//                             #body
//                         }
//                     }
//                 }
//             }
//             Frontmatter::Component(Component { signature }) => {
//                 let signature = signature.trim_end_matches(';');
//                 let signature: TokenStream =
//                     signature.parse().expect("Signature should be well-formed");
//
//                 let expressions = self.expressions.join("\n");
//                 let expressions: TokenStream = expressions
//                     .parse()
//                     .expect("Expressions should be well-formed");
//
//                 let body: TokenStream = self
//                     .body
//                     .parse()
//                     .expect("Generated component body should be well-formed");
//
//                 quote! {
//                     #[component]
//                     #signature {
//                         #expressions
//
//                         view! {
//                             #body
//                         }
//                     }
//                 }
//             }
//         }
//     }
// }
//
// pub fn generate_blog(articles: &[ParsedMarkdown]) -> String {
//     let links = articles
//         .iter()
//         .filter_map(|f| match &f.frontmatter {
//             Frontmatter::Blog(b) => Some(b),
//             Frontmatter::Component(_) => None,
//         })
//         .map(|blog| {
//             let name = &blog.title;
//             let link = format!("blog/{}", blog.slug());
//             quote! {
//                 <li>
//                     <a href=#link class="link-underline link-underline-2 text-2xl">
//                         #name
//                     </a>
//                 </li>
//             }
//         });
//
//     let blog_list = quote! {
//         #[component]
//         pub fn BlogList() -> impl IntoView {
//             view! {
//                 <ul>
//                     #(#links)*
//                 </ul>
//             }
//         }
//     };
//
//     let routes = articles
//         .iter()
//         .filter_map(|f| match &f.frontmatter {
//             Frontmatter::Blog(b) => Some(b),
//             Frontmatter::Component(_) => None,
//         })
//         .map(|blog| {
//             let slug = blog.slug();
//             let ident = blog.identifier();
//
//             quote! {
//                 Some(#slug) => {
//                     #ident().into_any()
//                 }
//             }
//         });
//
//     let blog_router = quote! {
//         #[component]
//         pub fn Blogs() -> impl IntoView {
//             let params = leptos_router::hooks::use_params_map();
//
//             move || {
//                 let param = params.read().get("id");
//                 match param.as_ref().map(|p| p.as_str()) {
//                     #(#routes,)*
//                     _ => view! {}.into_any(),
//                 }
//             }
//         }
//     };
//
//     let components = articles.iter().map(|c| c.to_component());
//
//     let file = quote! {
//         use leptos::prelude::*;
//
//         #blog_list
//         #blog_router
//         #(#components)*
//     };
//
//     let file: syn::File = syn::parse2(file).expect("Generated file should be well-formed");
//     prettyplease::unparse(&file)
// }
//
// fn collect<'a>(
//     nodes: impl IntoIterator<Item = &'a Node>,
//     partial: &mut PartialParse,
// ) -> anyhow::Result<String> {
//     Ok(nodes
//         .into_iter()
//         .map(|n| walk_tree(n, partial))
//         .collect::<anyhow::Result<Vec<_>>>()?
//         .join(""))
// }
//
// struct PartialParse {
//     frontmatter: Option<Frontmatter>,
//     expressions: Vec<String>,
//     include_katex_css: bool,
// }
//
// impl PartialParse {
//     fn new() -> Self {
//         Self {
//             frontmatter: None,
//             expressions: Vec::new(),
//             include_katex_css: false,
//         }
//     }
//
//     fn into_parsed(self, body: String) -> anyhow::Result<ParsedMarkdown> {
//         let frontmatter = self
//             .frontmatter
//             .ok_or(anyhow::Error::msg("No valid YAML frontmatter found."))?;
//
//         Ok(ParsedMarkdown {
//             frontmatter,
//             expressions: self.expressions,
//             body,
//             include_katex_css: self.include_katex_css,
//         })
//     }
// }
//
// fn walk_tree(node: &Node, partial: &mut PartialParse) -> anyhow::Result<String> {
//     Ok(match node {
//         Node::Root(Root { children, .. }) => {
//             let children = collect(children, partial)?;
//             format!("<div class=\"markdown\">{children}</div>")
//         }
//         Node::Heading(Heading {
//             depth, children, ..
//         }) => {
//             let children = collect(children, partial)?;
//             format!("<h{depth}>{children}</h{depth}>")
//         }
//         Node::Emphasis(Emphasis { children, .. }) => {
//             let children = collect(children, partial)?;
//             format!("<em>{children}</em>")
//         }
//         Node::Text(Text { value, .. }) => format!("\"{value}\""),
//         Node::Paragraph(Paragraph { children, .. }) => {
//             let children = collect(children, partial)?;
//             format!("<p>{children}</p>")
//         }
//         Node::Code(Code { value, lang, .. }) => {
//             match lang.as_deref() {
//                 Some("expr") => {
//                     partial.expressions.push(value.to_owned());
//                     String::new()
//                 }
//                 Some("inline") => value.to_owned(),
//                 Some(other) => {
//                     // TODO -- handle code hilighting
//
//                     let set = syntect::parsing::SyntaxSet::load_defaults_newlines();
//                     let reference = set
//                         .find_syntax_by_extension(other)
//                         .context(format!("\"{other}\" is not supported by syntect"))?;
//                     // let theme = syntect::highlighting::ThemeSet::load_defaults();
//                     // let theme = &theme.themes["Solarized (dark)"];
//
//                     let theme =
//                         syntect::highlighting::ThemeSet::load_from_folder("blog/code-themes")?;
//                     let theme = &theme.themes["halcyon"];
//
//                     let output = syntect::html::highlighted_html_for_string(
//                         &value, &set, &reference, theme,
//                     )?;
//
//                     format!("<div class=\"codeblock overflow-x-auto\" inner_html=r#\"{output}\"#.to_string() />")
//                 }
//                 None => {
//                     format!("<blockquote>{value}</blockquote>")
//                 }
//             }
//         }
//         Node::Break(_) => String::from("<br/>"),
//         Node::Blockquote(Blockquote { children, .. }) => {
//             let children = collect(children, partial)?;
//             format!("<blockquote>{children}</blockquote>")
//         }
//         Node::Definition(Definition { .. }) => todo!("definition not yet implemented"),
//         Node::Delete(Delete { children, .. }) => {
//             let children = collect(children, partial)?;
//             format!("<del>{children}<del>")
//         }
//         Node::Html(Html { value, .. }) => value.to_owned(),
//         Node::Image(Image { alt, url, .. }) => {
//             // TODO -- parse for scale
//             format!("{{ || corvusite_macros::image!(\"{url}\", \"{alt}\") }}")
//         }
//         Node::InlineCode(InlineCode { value, .. }) => format!("<code>{value}</code>"),
//         Node::InlineMath(InlineMath { value, .. }) => parser::sanitize_mathml(
//             &latex2mathml::latex_to_mathml(value, latex2mathml::DisplayStyle::Inline)?,
//         )?,
//         Node::Link(Link { children, url, .. }) => {
//             let children = collect(children, partial)?;
//             // TODO -- make configurable
//             let style = "link-underline link-underline-1";
//             format!("<a href=\"{url}\" class=\"{style}\" target=\"_blank\">{children}</a>")
//         }
//         Node::List(List {
//             children,
//             ordered,
//             start,
//             spread: _,
//             ..
//         }) => {
//             let children = collect(children, partial)?;
//             if *ordered {
//                 let start = start.unwrap_or(0);
//                 format!("<ol start=\"{start}\">{children}</ol>")
//             } else {
//                 format!("<ul>{children}</ul>")
//             }
//         }
//         Node::ListItem(ListItem { children, .. }) => {
//             let children = collect(children, partial)?;
//             format!("<li>{children}</li>")
//         }
//         Node::Math(Math { value, .. }) => {
//             //     parser::sanitize_mathml(&latex2mathml::latex_to_mathml(
//             //     value,
//             //     latex2mathml::DisplayStyle::Block,
//             // )?)?
//             partial.include_katex_css = true;
//             format!(
//                 "<div class=\"overflow-x-auto\" inner_html=r#\"{}\"#.to_string() />",
//                 katex::render_with_opts(
//                     &value,
//                     katex::Opts::builder().display_mode(true).build()?
//                 )?
//             )
//         }
//         Node::Strong(Strong { children, .. }) => {
//             let children = collect(children, partial)?;
//             format!("<strong>{children}</strong>")
//         }
//         Node::Yaml(Yaml { value, position }) => {
//             let position = position.clone().ok_or(anyhow::Error::msg(
//                 "Unable to extract YAML positional information",
//             ))?;
//             if position.start.line != 1 {
//                 anyhow::bail!(
//                     "Error at {}:{}: YAML is valid only as frontmatter",
//                     position.start.line,
//                     position.start.column
//                 )
//             }
//
//             let frontmatter: Frontmatter = serde_yaml::from_str(value)?;
//             partial.frontmatter = Some(frontmatter);
//             String::new()
//         }
//         _ => todo!(),
//     })
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn simple() {
//         let input = std::fs::read_to_string("examples/simple.md").unwrap();
//         let _output = ParsedMarkdown::from_markdown(&input).unwrap();
//     }
//
//     #[test]
//     fn test_output() {
//         let input = std::fs::read_to_string("examples/simple.md").unwrap();
//
//         let output = ParsedMarkdown::from_markdown(&input)
//             .unwrap()
//             .to_component();
//     }
// }
