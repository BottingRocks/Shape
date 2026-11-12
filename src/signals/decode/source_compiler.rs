//! Compatibility entry for the historical, conservative source fallback.
pub fn parse_salt(s: &str) -> Option<i64> {
    let s = s.rsplit('^').next()?.trim().trim_end_matches(')').trim();
    s.parse::<i64>().ok().or_else(|| {
        s.strip_prefix("0x")
            .and_then(|x| i64::from_str_radix(x, 16).ok())
    })
}
use crate::signals::{
    extract::contracts::{EmissionContract, GrammarNode},
    schema::SourceSpan,
    wire::*,
};
use serde_json::Value;
pub struct CompileContext<'a> {
    pub worker: bool,
    pub counts: &'a [Value],
}
pub fn compile(
    node: &GrammarNode,
    emission: &EmissionContract,
    algorithms: &AlgorithmReport,
    plan: &SourcePlan,
    source: &str,
    path: Vec<usize>,
    last: Option<(Vec<usize>, SourceSpan)>,
    facts: &mut Vec<Value>,
    context: &CompileContext<'_>,
) -> Result<Op, WireError> {
    super::direct::compile(
        node,
        emission,
        algorithms,
        plan,
        source,
        path,
        last,
        facts,
        &super::direct::CompileContext {
            direct: false,
            worker: context.worker,
            counts: context.counts,
            loops: Default::default(),
            last_count: Default::default(),
        },
    )
}
