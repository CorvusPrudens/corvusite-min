use super::{Child, Dom, NodeId};

impl Dom {
    pub fn output(&self, append_doctype: bool) -> String {
        let mut buffer = String::new();

        if append_doctype {
            buffer.push_str("<!DOCTYPE html>");
        }

        for child in self.nodes[self.root]
            .children
            .iter()
            .filter_map(|c| match c {
                Child::Node(n) => Some(n),
                _ => None,
            })
        {
            self.stringify_node(*child, &mut buffer);
        }

        buffer
    }

    fn stringify_node(&self, node: NodeId, buffer: &mut String) {
        let node = &self.nodes[node];

        buffer.push('<');
        buffer.push_str(&self.names[node.name].local);

        for attr in node.attributes.iter() {
            buffer.push(' ');
            buffer.push_str(&attr.name.local);
            buffer.push('=');
            buffer.push_str(&attr.value);
        }

        buffer.push('>');

        for child in &node.children {
            match child {
                Child::Node(n) => {
                    self.stringify_node(*n, buffer);
                }
                Child::Text(t) => {
                    buffer.push_str(t);
                }
            }
        }

        buffer.push_str("</");
        buffer.push_str(&self.names[node.name].local);
        buffer.push('>');
    }
}
