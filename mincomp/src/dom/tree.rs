use super::{Child, Dom, Node, NodeId, SharedDom};
use html5ever::tendril::StrTendril;
use html5ever::{
    interface::{NodeOrText, TreeSink},
    QualName,
};
use std::borrow::Cow;
use std::cell::Ref;

impl TreeSink for SharedDom {
    type Handle = NodeId;
    type Output = Dom;
    type ElemName<'a> = Ref<'a, QualName>;

    fn finish(self) -> Self::Output {
        Dom {
            root: self.document,
            nodes: self.nodes.into_inner(),
            names: self.names.into_inner(),
            errors: self.errors.into_inner(),
        }
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.errors.borrow_mut().push(msg);
    }

    fn get_document(&self) -> Self::Handle {
        self.document
    }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> Self::ElemName<'a> {
        let name = self
            .nodes
            .borrow()
            .get(*target)
            .expect("Target should exist in tree")
            .name;

        Ref::map(self.names.borrow(), move |inner| {
            inner.get(name).expect("Name should exist in tree")
        })
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<html5ever::Attribute>,
        _: html5ever::interface::ElementFlags,
    ) -> Self::Handle {
        let mut names = self.names.borrow_mut();
        let name = match names.iter().find_map(|(id, v)| (v == &name).then_some(id)) {
            Some(id) => id,
            None => names.insert(name),
        };

        self.nodes.borrow_mut().insert(Node {
            name,
            parent: None,
            children: vec![],
            attributes: attrs,
        })
    }

    fn create_comment(&self, _: html5ever::tendril::StrTendril) -> Self::Handle {
        Default::default()
    }

    fn create_pi(
        &self,
        _target: html5ever::tendril::StrTendril,
        _data: html5ever::tendril::StrTendril,
    ) -> Self::Handle {
        Default::default()
    }

    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut nodes = self.nodes.borrow_mut();
        let parent = nodes.get_mut(*parent).expect("Parent should exist in tree");

        match child {
            NodeOrText::AppendText(text) => {
                if let Some(Child::Text(t)) = parent.children.last_mut() {
                    t.push_tendril(&text);
                } else {
                    parent.children.push(Child::Text(text));
                }
            }
            NodeOrText::AppendNode(node) => {
                parent.children.push(Child::Node(node));
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        _element: &Self::Handle,
        _prev_element: &Self::Handle,
        _child: NodeOrText<Self::Handle>,
    ) {
        todo!("this is confusing")
    }

    fn append_before_sibling(
        &self,
        sibling: &Self::Handle,
        new_node: html5ever::interface::NodeOrText<Self::Handle>,
    ) {
        let mut nodes = self.nodes.borrow_mut();
        let sib_node = nodes.get(*sibling).expect("Sibling should exist in tree");
        let parent = sib_node.parent.expect("Sibling should have a parent");
        let parent = nodes.get_mut(parent).expect("Parent should exist in tree");

        let sib_index = parent
            .children
            .iter()
            .position(|c| c.node().is_some_and(|n| n == *sibling))
            .expect("Sibling should have index within parent");

        match sib_index {
            0 => {
                parent.children.insert(0, new_node.into());
            }
            _ => {
                if let Child::Text(t) = &mut parent.children[sib_index - 1] {
                    if let NodeOrText::AppendText(a) = &new_node {
                        t.push_tendril(a);
                        return;
                    }
                }

                parent.children.insert(sib_index, new_node.into());
            }
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
        // I think this doesn't matter for our purposes
    }

    fn get_template_contents(&self, _: &Self::Handle) -> Self::Handle {
        Default::default()
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _: html5ever::interface::QuirksMode) {}

    fn add_attrs_if_missing(&self, target: &Self::Handle, mut attrs: Vec<html5ever::Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        let target = nodes.get_mut(*target).expect("Target should exist in tree");

        attrs.retain(|a| {
            !target
                .attributes
                .iter()
                .any(|existing| existing.name == a.name)
        });
        target.attributes.append(&mut attrs);
    }

    fn remove_from_parent(&self, target: &Self::Handle) {
        let mut nodes = self.nodes.borrow_mut();

        let child = nodes.get_mut(*target).expect("Child should exist in tree");
        if let Some(parent) = child.parent.take() {
            let parent = nodes.get_mut(parent).expect("Parent should exist in tree");
            parent
                .children
                .retain(|c| !c.node().is_some_and(|n| n == *target));
        }
    }

    fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle) {
        let mut nodes = self.nodes.borrow_mut();

        let mut children = std::mem::take(
            &mut nodes
                .get_mut(*node)
                .expect("Original parent should exist in tree")
                .children,
        );
        for child in children.iter().filter_map(|c| c.node()) {
            let child = nodes.get_mut(child).unwrap();
            child.parent = Some(*new_parent);
        }

        let parent = nodes.get_mut(*new_parent).unwrap();
        parent.children.append(&mut children);
    }
}
