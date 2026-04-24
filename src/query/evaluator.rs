use crate::query::ast::{AttributeSelector, Combinator, CompoundSelector, QueryAst, SimpleSelector};

pub trait QueryTreeAdapter {
    type NodeId: Copy + Eq;

    fn children_of(&self, node: Self::NodeId) -> Vec<Self::NodeId>;

    fn matches_type(&self, _node: Self::NodeId, _type_name: &str) -> bool {
        false
    }

    fn matches_id(&self, _node: Self::NodeId, _id: &str) -> bool {
        false
    }

    fn matches_guid(&self, _node: Self::NodeId, _guid: &str) -> bool {
        false
    }

    fn matches_name(&self, _node: Self::NodeId, _name: &str) -> bool {
        false
    }

    fn matches_class(&self, _node: Self::NodeId, _class_name: &str) -> bool {
        false
    }

    fn matches_attribute(&self, _node: Self::NodeId, attribute: &AttributeSelector) -> bool {
        if attribute.name == "name" {
            return attribute
                .value
                .as_deref()
                .map(|name| self.matches_name(_node, name))
                .unwrap_or(false);
        }
        false
    }
}

#[derive(Debug, Default)]
pub struct QueryEvaluator;

impl QueryEvaluator {
    pub fn evaluate<T: QueryTreeAdapter>(
        tree: &T,
        root: T::NodeId,
        ast: &QueryAst,
    ) -> Vec<T::NodeId> {
        let mut out = Vec::new();

        for sequence in &ast.selector_groups {
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                if Self::matches_sequence(tree, node, sequence) {
                    out.push(node);
                }

                let children = tree.children_of(node);
                for child in children.into_iter().rev() {
                    stack.push(child);
                }
            }
        }

        out
    }

    fn matches_sequence<T: QueryTreeAdapter>(
        tree: &T,
        node: T::NodeId,
        sequence: &crate::query::ast::SelectorSequence,
    ) -> bool {
        let Some(last) = sequence.segments.last() else {
            return false;
        };

        if !Self::matches_compound(tree, node, &last.compound) {
            return false;
        }

        sequence
            .segments
            .iter()
            .all(|segment| match segment.combinator {
                Some(Combinator::Child) | Some(Combinator::Descendant) | None => true,
            })
    }

    fn matches_compound<T: QueryTreeAdapter>(
        tree: &T,
        node: T::NodeId,
        compound: &CompoundSelector,
    ) -> bool {
        compound
            .simple_selectors
            .iter()
            .all(|selector| Self::matches_simple(tree, node, selector))
    }

    fn matches_simple<T: QueryTreeAdapter>(
        tree: &T,
        node: T::NodeId,
        selector: &SimpleSelector,
    ) -> bool {
        match selector {
            SimpleSelector::Universal => true,
            SimpleSelector::Type(type_name) => tree.matches_type(node, type_name),
            SimpleSelector::Id(id) => tree.matches_id(node, id),
            SimpleSelector::Guid(guid) => tree.matches_guid(node, guid),
            SimpleSelector::Name(name) => tree.matches_name(node, name),
            SimpleSelector::Class(class_name) => tree.matches_class(node, class_name),
            SimpleSelector::Attribute(attribute) => tree.matches_attribute(node, attribute),
        }
    }
}