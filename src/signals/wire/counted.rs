//! Invertible presentation of source-certified counted data arrays.
//! Rules are derived from executable proofs, never field names or observations.
use super::{
    named::{Field, Node, Step},
    CountExpr, CountRef, Op, Program,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct Rule {
    pub leaf: usize,
    pub control: Vec<Step>,
    pub maximum: usize,
}
#[derive(Clone, Default)]
pub(super) struct Rules(pub Vec<Rule>);

fn steps(path: &super::ControlPath) -> Vec<Step> {
    path.iter()
        .map(|s| match s {
            super::ControlStep::Child(i) => Step::Child(*i),
            super::ControlStep::Arm(i) => Step::Arm(*i),
            super::ControlStep::Body => Step::Body,
        })
        .collect()
}
fn numeric(path: &[Step]) -> Vec<usize> {
    path.iter()
        .map(|s| match s {
            Step::Child(i) | Step::Arm(i) => *i,
            Step::Body => 0,
        })
        .collect()
}
fn data(n: &Node) -> bool {
    match n {
        Node::Leaf { .. } => true,
        Node::Record { .. } | Node::ScalarNull { .. } => true,
        Node::FixedArray { items } => items.iter().all(|f| data(&f.node)),
        Node::Choice { arms, .. } => !arms.is_empty() && arms.iter().all(data),
        Node::Array { item, .. } => data(item),
        // No-write/packed/optional item layouts do not prove a displayed length.
        _ => false,
    }
}
impl Rules {
    pub fn new(program: &Program, fields: &[Field]) -> Self {
        fn proofs(op: &Op, refs: &mut Vec<CountRef>, uses: &mut BTreeMap<Vec<usize>, usize>) {
            match op {
                Op::CountedRepeat {
                    count: CountExpr::PriorNumber { reference },
                    body,
                    ..
                } => {
                    *uses.entry(reference.path.clone()).or_default() += 1;
                    refs.push(reference.clone());
                    proofs(body, refs, uses);
                }
                Op::CounterChoice { proof, arms } => {
                    if let super::CounterInput::Packed { count, .. } = &proof.input {
                        *uses.entry(count.path.clone()).or_default() += 1;
                    }
                    for a in arms {
                        proofs(a, refs, uses);
                    }
                }
                Op::Sequence { items } => {
                    for n in items {
                        proofs(n, refs, uses);
                    }
                }
                Op::Choice { arms, .. } | Op::DomainChoice { arms, .. } => {
                    for n in arms {
                        proofs(n, refs, uses);
                    }
                }
                Op::Repeat { body, .. } => proofs(body, refs, uses),
                Op::RootChoice { cases } => {
                    for c in cases {
                        if let super::Support::Supported { program }
                        | super::Support::RequiresInitializer { program, .. } = &*c.support
                        {
                            proofs(&program.body, refs, uses);
                        }
                    }
                }
                _ => {}
            }
        }
        fn walk(
            fields: &[Field],
            refs: &[CountRef],
            uses: &BTreeMap<Vec<usize>, usize>,
            rules: &mut Rules,
        ) {
            for array in fields {
                if let Node::Array { control, item } = &array.node {
                    if array.bits.is_none() && data(item) {
                        let found: Vec<_> = refs
                            .iter()
                            .filter(|r| {
                                steps(&r.repeat_path) == *control && uses.get(&r.path) == Some(&1)
                            })
                            .collect();
                        if let [r] = found.as_slice() {
                            let counts: Vec<_> = fields.iter().filter_map(|f| match &f.node {
                                Node::Leaf { leaf, grammar_path, kind, .. } if f.bits.is_none() && kind == "number" && numeric(grammar_path) == r.path
                                    && grammar_path.split_last().zip(control.split_last()).is_some_and(|((last, parent), (repeat, scope))| parent == scope && matches!((last,repeat), (Step::Child(a),Step::Child(b)) if a < b)) => Some(*leaf),
                                _ => None,
                            }).collect();
                            if let [leaf] = counts.as_slice() {
                                rules.0.push(Rule {
                                    leaf: *leaf,
                                    control: control.clone(),
                                    maximum: r.maximum,
                                });
                            }
                        }
                    }
                }
                fn node(
                    n: &Node,
                    refs: &[CountRef],
                    uses: &BTreeMap<Vec<usize>, usize>,
                    rules: &mut Rules,
                ) {
                    match n {
                        Node::Record { fields } => walk(fields, refs, uses, rules),
                        Node::Array { item, .. } => node(item, refs, uses, rules),
                        Node::Choice { arms, .. } => {
                            for a in arms {
                                node(a, refs, uses, rules);
                            }
                        }
                        _ => {}
                    }
                }
                node(&array.node, refs, uses, rules);
            }
        }
        let (mut refs, mut uses) = (vec![], BTreeMap::new());
        proofs(&program.body, &mut refs, &mut uses);
        let mut rules = Self::default();
        walk(fields, &refs, &uses, &mut rules);
        rules
    }
    pub fn count(&self, n: &Node) -> Option<&Rule> {
        if let Node::Leaf { leaf, .. } = n {
            self.0.iter().find(|r| r.leaf == *leaf)
        } else {
            None
        }
    }
    pub fn array(&self, n: &Node) -> Option<&Rule> {
        if let Node::Array { control, .. } = n {
            self.0.iter().find(|r| r.control == *control)
        } else {
            None
        }
    }
    pub fn collapsed<'a>(&self, fields: &'a [Field]) -> Option<&'a Field> {
        if fields.len() == 2 && fields.iter().any(|f| self.count(&f.node).is_some()) {
            fields.iter().find(|f| self.array(&f.node).is_some())
        } else {
            None
        }
    }
}
