use html5ever::QualName;

use super::{Child, Dom, Node, NodeId};

impl Dom {
    /// Move the root node to the first child of the body.
    pub fn make_component(&mut self) {
        let body = self.find_map(|name, _, id| {
            if &name.local == "body" {
                Some(id)
            } else {
                None
            }
        });

        if let Some(body) = body {
            self.root = body;
        }
    }

    /// Descent the tree depth-first.
    pub fn find_map<F, O>(&self, mut visitor: F) -> Option<O>
    where
        F: FnMut(&QualName, &Node, NodeId) -> Option<O>,
    {
        self.find_map_recursive(&mut visitor, self.root)
    }

    fn find_map_recursive<F, O>(&self, visitor: &mut F, id: NodeId) -> Option<O>
    where
        F: FnMut(&QualName, &Node, NodeId) -> Option<O>,
    {
        let node = &self.nodes[id];
        if let Some(s) = visitor(&self.names[node.name], node, id) {
            return Some(s);
        };

        for child in node.children.iter().filter_map(|c| match c {
            Child::Node(n) => Some(n),
            _ => None,
        }) {
            if let Some(s) = self.find_map_recursive(visitor, *child) {
                return Some(s);
            }
        }

        None
    }

    fn append(&mut self, parent: NodeId, child: Child) {
        let parent = &mut self.nodes[parent];

        match child {
            Child::Text(text) => {
                if let Some(Child::Text(t)) = parent.children.last_mut() {
                    t.push_tendril(&text);
                } else {
                    parent.children.push(Child::Text(text));
                }
            }
            Child::Node(node) => {
                parent.children.push(Child::Node(node));
            }
        }
    }
}
