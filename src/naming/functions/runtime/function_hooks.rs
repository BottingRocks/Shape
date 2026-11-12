//! Naming rule for the function-valued property descriptor interceptor. The
//! whole-body proof lives in `signals::analysis::roles::function_interceptor`.
use super::*;
use crate::signals::analysis::roles::function_interceptor::{
    function_interceptor, assignment_identifier, assignment_member, block, call_of, direct_call, exact_arguments, increments, is_break, is_continue, is_null_compare, is_null_literal, is_return_null, literal_object, member_on, plain_assignment, plain_local_in_body, plain_local_symbol, symbol_of,
};

#[cfg(test)]
use crate::signals::analysis::roles::function_interceptor::{loop_body, scan_statements, Scan};

#[path = "function_hooks_transport.rs"]
mod transport;

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function_interceptor(model, site, function, body).is_none() {
        if transport::proves(model, site).is_some() {
            return vec![proposal(
                transport::NAME,
                "owned_method_interceptor_external_helpers",
                "closed ordered own installer grammar with actual lexical capture initialization before invocation/escape: inline descriptor walk, strict value/frozen refusals, null-prototype originals, same-owner keyed cache with dead reset and post-loop record copy, exact distinct before/after hook publication, owned strict self-instance wrapper with effective-args/bypass/throw/finally flow, exact guarded property copy and descriptor/registry publication; call-through, hook factory, invoke/construct, source-map and exclusion captures are external consumer inputs; native helper identity and stronger specialized purpose explicitly unresolved",
                f,
            )];
        }
        if transport::proves_lowered(model, site).is_some() {
            return vec![proposal("installMethodInterceptorWithExternalHelpers", "owned_method_interceptor_lowered_external_helpers", "closed ordered installer grammar with scoped declaration transport: exact prototype walk or independently resolved immutable descriptor helper; exact keyed cache scan or independently resolved immutable same-registry helper; fully consumed record/boolean aliases, local writer/read inventories and initialization; exact frozen refusal and strict wrapper payload field order, effective arguments, receiver/self-instance constructor split, bypass/throw/finally flow; source mapping, protected property copying, descriptor-before-registry publication and returned hooks. Actual lexical capture initialization and exposure frontier remain required; call-through, hook factory and native invoke/construct/source consumers remain explicitly external", f)];
        }
        return Vec::new();
    }
    vec![proposal(
        "installMethodInterceptor",
        "posthost_function_hooks_interceptor",
        "whole owned body: function-valued property descriptor lookup and null guard, containingObj/desc/configurable/writable/value read, own-value guard, null-prototype originals record, exact configurable/writable/typeof early return, cached-original shortcut, two zero-argument hook factories consumed by a directly owned strict wrapper that notifies both hooks in try/finally and honours bypassResult, descriptor value re-installation, Function.prototype.toString capture, for/while own-property descriptor copy guarded by indexOf, prototype guard, exact containing-object defineProperty, onBeforeInvoke/onAfterInvoke/originals result record, keyed registry push and result return; descriptor lookup and cache lookup are the opaque two-argument helper call or the equivalent inline prototype walk / per-property registry scan, the hook factory is a consumer protocol input, local spellings and statement order are not evidence",
        f,
    )]
}
