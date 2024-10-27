use crate::element::{Element, Node};
use winnow::{
    ascii::multispace0,
    combinator::delimited,
    error::{ContextError, ParseError},
    Parser,
};

pub mod element;
pub mod parse;

pub struct Document<'s> {
    pub nodes: Vec<Node<'s>>,
}

pub struct Component<'s> {
    pub root: Element<'s>,
}

impl<'s> Component<'s> {
    pub fn new(mut source: &'s str) -> Result<Self, ParseError<&'s str, ContextError>> {
        let root = delimited(multispace0, parse::element, multispace0).parse(&mut source)?;

        Ok(Self { root })
    }
}

impl<'s> Document<'s> {
    pub fn new(mut source: &'s str) -> Result<Self, ParseError<&'s str, ContextError>> {
        let nodes = parse::nodes.parse(&mut source)?;

        Ok(Self { nodes })
    }

    pub fn expand(&mut self, components: &[Component<'s>]) {
        loop {
            if !Self::expand_recurse(&mut self.nodes, components) {
                break;
            }
        }
    }

    fn expand_recurse(nodes: &mut Vec<Node<'s>>, components: &[Component<'s>]) -> bool {
        let mut mutated = false;
        // if let Some(component) = components.iter().find(|c| c.name == element.name) {
        //     mutated = true;
        //     let mut children = std::mem::take(&mut element.children);
        //     *element = component.clone();
        //     element.name = "div";
        //
        //     let mut index = 0;
        //     let outlet = element::find_mut(element, &mut |el| {
        //         if let Some(i) = el
        //             .children
        //             .iter()
        //             .filter_map(|c| c.element())
        //             .position(|p| p.name == "children")
        //         {
        //             index = i;
        //             true
        //         } else {
        //             false
        //         }
        //     });
        //
        //     if let Some(outlet) = outlet {
        //         outlet.children.remove(index);
        //         outlet.children.append(&mut children)
        //     }
        // }

        let mut index = 0;
        while index < nodes.len() {
            let Some(child) = nodes[index].element_mut() else {
                index += 1;
                continue;
            };

            if let Some(component) = components.iter().find(|c| c.root.name == child.name) {
                mutated = true;
                let declared_attributes = &component.root.attributes;
                let mut replacement_attributes = Vec::with_capacity(declared_attributes.len());

                for attribute in declared_attributes {
                    if let Some(attr) = child.attributes.iter().find(|a| a.name == attribute.name) {
                        replacement_attributes.push(*attr);
                    } else {
                        replacement_attributes.push(*attribute);
                    }
                }

                let mut component_copy = component.root.clone();

                // Assign properties
                element::walk(&mut component_copy, &mut |element| {
                    for attr in element.attributes.iter_mut() {
                        if let Some(value) = replacement_attributes.iter().find_map(|a| {
                            attr.value.is_some_and(|v| v == a.name).then_some(a.value)
                        }) {
                            attr.value = value;
                        }
                    }
                });

                let mut children = std::mem::take(&mut child.children);

                let mut inner_index = 0;
                let outlet = element::find_mut(&mut component_copy, &mut |el| {
                    if let Some(i) = el
                        .children
                        .iter()
                        .filter_map(|c| c.element())
                        .position(|p| p.name == "children")
                    {
                        inner_index = i;
                        true
                    } else {
                        false
                    }
                });

                if let Some(outlet) = outlet {
                    outlet.children.remove(inner_index);
                    outlet.children.append(&mut children)
                }

                nodes.remove(index);

                for (i, child) in component_copy.children.into_iter().enumerate() {
                    nodes.insert(index + i, child);
                }
            }

            // TODO: technically this can panic if the component has no children
            let Some(child) = nodes[index].element_mut() else {
                index += 1;
                continue;
            };

            mutated |= Self::expand_recurse(&mut child.children, components);

            index += 1;
        }

        // for child in element.children.iter_mut().filter_map(|c| c.element_mut()) {
        //     if let Some(component) = components.iter().find(|c| c.name == child.name) {
        //         mutated = true;
        //         let mut children = std::mem::take(&mut element.children);
        //         *element = component.clone();
        //         element.name = "div";
        //
        //         let mut index = 0;
        //         let outlet = element::find_mut(element, &mut |el| {
        //             if let Some(i) = el
        //                 .children
        //                 .iter()
        //                 .filter_map(|c| c.element())
        //                 .position(|p| p.name == "children")
        //             {
        //                 index = i;
        //                 true
        //             } else {
        //                 false
        //             }
        //         });
        //
        //         if let Some(outlet) = outlet {
        //             outlet.children.remove(index);
        //             outlet.children.append(&mut children)
        //         }
        //     }
        //
        //     mutated |= Self::expand_recurse(child, components);
        // }

        mutated
    }
}

impl Document<'_> {
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write("<!DOCTYPE html>".as_bytes())?;
        self.write_element(writer, &self.nodes)
    }

    fn write_element<W: std::io::Write>(
        &self,
        writer: &mut W,
        nodes: &[Node<'_>],
    ) -> std::io::Result<()> {
        for node in nodes {
            match node {
                Node::Element(element) => {
                    writer.write(&[b'<'])?;
                    writer.write(element.name.as_bytes())?;

                    for attribute in element.attributes.iter() {
                        writer.write(&[b' '])?;
                        writer.write(attribute.name.as_bytes())?;
                        if let Some(value) = attribute.value {
                            writer.write(r#"=""#.as_bytes())?;
                            writer.write(value.as_bytes())?;
                            writer.write(&[b'"'])?;
                        }
                    }

                    if element.children.is_empty() {
                        writer.write("/>".as_bytes())?;
                    } else {
                        writer.write(&[b'>'])?;

                        self.write_element(writer, &element.children)?;

                        writer.write("</".as_bytes())?;
                        writer.write(element.name.as_bytes())?;
                        writer.write(&[b'>'])?;
                    }
                }
                Node::Text(t) => {
                    writer.write(t.as_bytes())?;
                }
                Node::Comment(_) => {}
            }
        }

        Ok(())
    }
}
