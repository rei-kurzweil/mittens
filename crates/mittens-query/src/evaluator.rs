use crate::ast::{
    AttributeSelector, Combinator, CompoundSelector, QueryAst, SelectorSequence, SimpleSelector,
};

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
    /// Evaluate `ast` against `tree` rooted at `root`. Returns matches in
    /// DFS pre-order, including `root` itself if it matches.
    pub fn evaluate<T: QueryTreeAdapter>(
        tree: &T,
        root: T::NodeId,
        ast: &QueryAst,
    ) -> Vec<T::NodeId> {
        let mut out = Vec::new();
        for sequence in &ast.selector_groups {
            let mut path: Vec<T::NodeId> = Vec::new();
            collect(tree, root, sequence, &mut path, &mut out);
        }
        out
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

fn collect<T: QueryTreeAdapter>(
    tree: &T,
    node: T::NodeId,
    sequence: &SelectorSequence,
    path: &mut Vec<T::NodeId>,
    out: &mut Vec<T::NodeId>,
) {
    path.push(node);
    if matches_with_path::<T>(tree, sequence, path) {
        out.push(node);
    }
    for child in tree.children_of(node) {
        collect(tree, child, sequence, path, out);
    }
    path.pop();
}

/// `path` is the chain of ancestors from the query root down to (and including)
/// the candidate node. The candidate is `path[path.len() - 1]`.
fn matches_with_path<T: QueryTreeAdapter>(
    tree: &T,
    sequence: &SelectorSequence,
    path: &[T::NodeId],
) -> bool {
    let segs = &sequence.segments;
    if segs.is_empty() || path.is_empty() {
        return false;
    }
    let last = segs.len() - 1;
    let candidate = path[path.len() - 1];
    if !QueryEvaluator::matches_compound(tree, candidate, &segs[last].compound) {
        return false;
    }

    // Walk earlier segments backward through ancestors in `path`.
    // The combinator on segs[i+1] describes how segs[i+1] relates to segs[i].
    let mut path_cursor: isize = path.len() as isize - 1;
    for i in (0..last).rev() {
        let combinator = segs[i + 1].combinator.unwrap_or(Combinator::Descendant);
        match combinator {
            Combinator::Child => {
                path_cursor -= 1;
                if path_cursor < 0 {
                    return false;
                }
                if !QueryEvaluator::matches_compound(
                    tree,
                    path[path_cursor as usize],
                    &segs[i].compound,
                ) {
                    return false;
                }
            }
            Combinator::Descendant => {
                path_cursor -= 1;
                let mut found = false;
                while path_cursor >= 0 {
                    if QueryEvaluator::matches_compound(
                        tree,
                        path[path_cursor as usize],
                        &segs[i].compound,
                    ) {
                        found = true;
                        break;
                    }
                    path_cursor -= 1;
                }
                if !found {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{CompoundSelector, SelectorSegment, SelectorSequence, SimpleSelector};

    /// Toy tree adapter for combinator tests.
    /// Nodes are indices into a fixed slice of `(parent, type, name)`.
    struct ToyTree {
        nodes: Vec<(Option<usize>, &'static str, &'static str)>,
    }

    impl QueryTreeAdapter for ToyTree {
        type NodeId = usize;
        fn children_of(&self, node: usize) -> Vec<usize> {
            self.nodes
                .iter()
                .enumerate()
                .filter(|(_, (p, _, _))| *p == Some(node))
                .map(|(i, _)| i)
                .collect()
        }
        fn matches_type(&self, node: usize, type_name: &str) -> bool {
            self.nodes[node].1 == type_name
        }
        fn matches_name(&self, node: usize, name: &str) -> bool {
            self.nodes[node].2 == name
        }
    }

    fn type_seg(t: &'static str, comb: Option<Combinator>) -> SelectorSegment {
        SelectorSegment {
            combinator: comb,
            compound: CompoundSelector {
                simple_selectors: vec![SimpleSelector::Type(t.into())],
            },
        }
    }

    /// Build tree:
    ///   0 root(Root)
    ///   ├─ 1 a(A)
    ///   │  └─ 2 b(B)
    ///   │     └─ 3 c(C)
    ///   └─ 4 a2(A)
    ///      └─ 5 c(C)            // C direct child of A
    fn build_tree() -> ToyTree {
        ToyTree {
            nodes: vec![
                (None, "Root", "root"),
                (Some(0), "A", "a"),
                (Some(1), "B", "b"),
                (Some(2), "C", "c"),
                (Some(0), "A", "a2"),
                (Some(4), "C", "c2"),
            ],
        }
    }

    #[test]
    fn child_combinator_filters_indirect_descendants() {
        let tree = build_tree();
        let ast = QueryAst {
            selector_groups: vec![SelectorSequence {
                segments: vec![type_seg("A", None), type_seg("C", Some(Combinator::Child))],
            }],
        };
        let matches = QueryEvaluator::evaluate(&tree, 0, &ast);
        // C node 3 is a grandchild of A — should NOT match A > C.
        // C node 5 is a direct child of A — should match.
        assert_eq!(matches, vec![5]);
    }

    #[test]
    fn descendant_combinator_matches_at_any_depth() {
        let tree = build_tree();
        let ast = QueryAst {
            selector_groups: vec![SelectorSequence {
                segments: vec![
                    type_seg("A", None),
                    type_seg("C", Some(Combinator::Descendant)),
                ],
            }],
        };
        let mut matches = QueryEvaluator::evaluate(&tree, 0, &ast);
        matches.sort();
        assert_eq!(matches, vec![3, 5]);
    }

    #[test]
    fn single_segment_matches_all_descendants() {
        let tree = build_tree();
        let ast = QueryAst {
            selector_groups: vec![SelectorSequence {
                segments: vec![type_seg("A", None)],
            }],
        };
        let mut matches = QueryEvaluator::evaluate(&tree, 0, &ast);
        matches.sort();
        assert_eq!(matches, vec![1, 4]);
    }
}
