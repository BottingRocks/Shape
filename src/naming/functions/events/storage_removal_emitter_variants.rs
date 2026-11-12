//! Additional exact frame grammars for `emitStorageRemovalSignal`.
//!
//! The existing evaluator remains first. This fallback permits independently
//! lowered holder/direct cursor branches only after both complete frames prove
//! identical captured state, flag lane order, factories, seed literal and tag.
//! It never obtains evidence from a sibling or a nested donor. Factory contracts
//! remain the parent's resolved, immediately-owned returned-child proofs.
use super::storage_removal_emitter::{
    alias_declaration, arity, assigned_symbol, assignment, binary, block, call,
    captured_flags_expression, captured_local, computed, cursor_declaration,
    cursor_writer_declaration, direct_writer_branch, distinct, error_state, expression_statement,
    fixed_null_branch, header_declaration, is_null, member, member_target, next_frame, null_branch,
    number, removal_state, reseed_declaration, seed_declaration, storage_guard, stream_call,
    symbol, writer_declaration, plain_local, pure_writer_declaration,
};
use super::*;
use oxc_semantic::SymbolId;
type Symbol = SymbolId;

#[derive(Clone, Copy, PartialEq)]
struct VariantFrame {
    old: Symbol,
    observer: Symbol,
    lanes: [Symbol; 3],
    seed_factory: Symbol,
    writer_factory: Symbol,
    reseed: f64,
    tag: f64,
    fixed: bool,
}

#[derive(Clone, Copy)]
enum Layout {
    FixedHolder,
    Holder,
    Direct,
    AliasHolder,
    AliasDirect,
    SplitHolder,
    FixedPort,
}

/// Keep metadata for independent old proof routes byte-exact.
pub(in crate::naming) enum Route {
    Existing,
    AliasedDirect,
    Extended,
    FixedPort,
}

/// Only the emitted holder's exact `[writer, old, header]` tuple is callable.
/// `.call(void 0, ...)` must have a pure receiver expression and exact arity.
fn holder_call(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    writer: Symbol,
    offset: f64,
) -> Option<()> {
    let invocation = call(expression)?;
    let (callee, args) = if let Some(receiver) = member(&invocation.callee, "call") {
        let args = arity(invocation, 4)?;
        let Expression::UnaryExpression(void) = args[0].without_parentheses() else {
            return None;
        };
        if void.operator.as_str() != "void" || !number(&void.argument, 0.0) {
            return None;
        }
        (receiver, args[1..].to_vec())
    } else {
        (&invocation.callee, arity(invocation, 3)?)
    };
    let (object, index) = computed(callee)?;
    if symbol(model, object) != Some(writer) || !number(index, 0.0) {
        return None;
    }
    for (arg, index) in args[..2].iter().zip([1.0, 2.0]) {
        let (object, slot) = computed(arg)?;
        if symbol(model, object) != Some(writer) || !number(slot, index) {
            return None;
        }
    }
    number(args[2], offset).then_some(())
}

fn holder_tuple(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    writer: Symbol,
    old: Symbol,
    header: Symbol,
) -> Option<()> {
    let store = assignment(expression_statement(statement)?)?;
    let Expression::ArrayExpression(array) = store.right.without_parentheses() else {
        return None;
    };
    let [first, second, third] = array.elements.as_slice() else {
        return None;
    };
    (assigned_symbol(model, store) == Some(writer)
        && symbol(model, first.as_expression()?) == Some(writer)
        && symbol(model, second.as_expression()?) == Some(old)
        && symbol(model, third.as_expression()?) == Some(header))
    .then_some(())
}

/// Exactly one frame: four setup statements, optional immutable low-lane
/// alias, old-value snapshot, optional cursor, two-armed null test and next.
/// Fixed frames place flags before the snapshot; variable frames append them
/// after the branch. No effects or nested declarations are skipped.
fn frame(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
) -> Option<(VariantFrame, Layout)> {
    let [seed_stmt, header_stmt, reseed_stmt, writer_stmt, rest @ ..] = statements else {
        return None;
    };
    let seed = seed_declaration(model, seed_stmt)?;
    let header = header_declaration(model, header_stmt, seed)?;
    let (seed_factory, reseed) = reseed_declaration(model, site, reseed_stmt, seed)?;

    // Fixed holder: flags; old; if(null){tag at 5}else{tuple; call at 4}; next.
    // Variable forms: [low alias]; old; [cursor]; if; flags; next.
    let fixed = rest.first().and_then(expression_statement).is_some();
    let (low_alias, old_stmt, cursor_stmt, branch_stmt, flags_stmt, next) = if fixed {
        let [flags, old, branch, next] = rest else {
            return None;
        };
        (None, old, None, branch, flags, next)
    } else {
        match rest {
            [old, branch @ Statement::IfStatement(_), flags, next] => {
                (None, old, None, branch, flags, next)
            }
            [old, cursor, branch @ Statement::IfStatement(_), flags, next]
                if one_variable(cursor)?.init.is_none() =>
            {
                (None, old, Some(cursor), branch, flags, next)
            }
            [low, old, branch @ Statement::IfStatement(_), flags, next] => {
                (Some(low), old, None, branch, flags, next)
            }
            [low, old, cursor, branch @ Statement::IfStatement(_), flags, next] => {
                (Some(low), old, Some(cursor), branch, flags, next)
            }
            _ => return None,
        }
    };
    let separate = cursor_stmt.is_some();
    let (writer, writer_factory) = if fixed || separate {
        writer_declaration(model, site, writer_stmt, seed, header)?
    } else {
        cursor_writer_declaration(model, site, writer_stmt, seed, header)?
    };
    if seed_factory == writer_factory {
        return None;
    }
    let (old, old_source) = alias_declaration(model, site, old_stmt, &[seed, header, writer])?;
    let low_alias = match low_alias {
        Some(stmt) => Some(alias_declaration(
            model,
            site,
            stmt,
            &[seed, header, writer, old],
        )?),
        None => None,
    };
    let cursor = if let Some(stmt) = cursor_stmt {
        let mut excluded = vec![seed, header, writer, old];
        if let Some((alias, _)) = low_alias {
            excluded.push(alias);
        }
        cursor_declaration(model, stmt, &excluded)?
    } else {
        writer
    };

    let Statement::IfStatement(branch) = branch_stmt else {
        return None;
    };
    let (left, right) = binary(&branch.test, "==").or_else(|| binary(&branch.test, "==="))?;
    if symbol(model, left) != Some(old) || !is_null(right) {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let no = block(branch.alternate.as_ref()?)?;
    let tag = if fixed {
        let tag = fixed_null_branch(model, header, seed, yes)?;
        let [tuple, invoke] = no else {
            return None;
        };
        holder_tuple(model, tuple, writer, old, header)?;
        holder_call(model, expression_statement(invoke)?, writer, 4.0)?;
        tag
    } else {
        let tag = null_branch(model, header, seed, cursor, yes)?;
        if separate {
            let [tuple, position] = no else {
                return None;
            };
            holder_tuple(model, tuple, writer, old, header)?;
            let position = assignment(expression_statement(position)?)?;
            if assigned_symbol(model, position) != Some(cursor) {
                return None;
            }
            holder_call(model, &position.right, writer, 3.0)?;
        } else {
            direct_writer_branch(model, writer, old, header, no)?;
        }
        tag
    };
    let append = assignment(expression_statement(flags_stmt)?)?;
    let (object, index) = member_target(&append.left)?;
    if symbol(model, object) != Some(header) {
        return None;
    }
    if fixed {
        if !number(index, 4.0) {
            return None;
        }
    } else {
        let (position, one) = binary(index, "+")?;
        if symbol(model, position) != Some(cursor) || !number(one, 1.0) {
            return None;
        }
    }
    let (flags, stream) = binary(&append.right, "^")?;
    if !stream_call(model, stream, seed) {
        return None;
    }
    let (mut low, middle, high) = captured_flags_expression(model, flags)?;
    if let Some((alias, source)) = low_alias {
        if low != alias {
            return None;
        }
        low = source;
    }
    let lanes = [low, middle, high];
    for lane in lanes {
        captured_local(model, site, lane)?;
    }
    let layout = match (fixed, separate, low_alias.is_some()) {
        (true, _, _) => Layout::FixedHolder,
        (false, true, false) => Layout::Holder,
        (false, false, false) => Layout::Direct,
        (false, true, true) => Layout::AliasHolder,
        (false, false, true) => Layout::AliasDirect,
    };
    Some((
        VariantFrame {
            old: old_source,
            observer: next_frame(model, site, header, next)?,
            lanes,
            seed_factory,
            writer_factory,
            reseed,
            tag,
            fixed,
        },
        layout,
    ))
}

pub(in crate::naming) fn storage_removal(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    body: &FunctionBody<'_>,
) -> Option<Route> {
    let [Statement::TryStatement(try_)] = body.statements.as_slice() else {
        return None;
    };
    if try_.finalizer.is_some() {
        return None;
    }
    let [Statement::IfStatement(guard)] = try_.block.body.as_slice() else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let key = captured_local(model, site, storage_guard(model, &guard.test, input)?)?;
    let [removal, old, success_tail @ ..] = block(&guard.consequent)? else {
        return None;
    };
    let (removal, old) = removal_state(model, site, input, removal, old)?;
    let handler = try_.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(caught) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    if model.binding_writes(caught.symbol_id.get()?) != 0 {
        return None;
    }
    let [error, catch_tail @ ..] = handler.body.body.as_slice() else {
        return None;
    };
    let error = error_state(model, site, error)?;
    let (success, success_layout) = frame(model, site, success_tail).or_else(|| split_holder_frame(model, site, success_tail)).or_else(|| fixed_port_frame(model, site, success_tail))?;
    let (catch, catch_layout) = frame(model, site, catch_tail).or_else(|| split_holder_frame(model, site, catch_tail)).or_else(|| fixed_port_frame(model, site, catch_tail))?;
    // Each topology fixes its own wire order and removal/error bit positions.
    // Unaliased direct/direct keeps the parent's removal/error order (1, 2).
    // Do not reinterpret a two-arm lane swap as a new successful proof: equal
    // frames alone cannot establish the observer's expected wire schema.
    // Alias-direct/alias-direct also writes old-value first, but its immutable
    // low alias is removal and its middle lane is error. Neither pair permits
    // moving the flags ahead of the old-value writer or swapping frame lanes.
    let (removal_lane, error_lane) = match (success_layout, catch_layout) {
        (Layout::FixedPort, Layout::FixedPort) => (0, 1),
        (Layout::SplitHolder, Layout::SplitHolder) => (0, 2),
        (Layout::FixedHolder, Layout::FixedHolder) => (1, 2),
        (Layout::Holder, Layout::Direct) => (0, 1),
        (Layout::AliasDirect, Layout::AliasHolder) => (1, 0),
        (Layout::AliasDirect, Layout::AliasDirect) => (0, 1),
        _ => return None,
    };
    if success != catch
        || success.old != old
        || success.lanes[removal_lane] != removal
        || success.lanes[error_lane] != error
    {
        return None;
    }
    let state = [
        key,
        old,
        success.observer,
        success.lanes[0],
        success.lanes[1],
        success.lanes[2],
    ];
    if !distinct(&state)
        || state.contains(&success.seed_factory)
        || state.contains(&success.writer_factory)
    {
        return None;
    }
    Some(match (success_layout, catch_layout) {
        (Layout::FixedPort, Layout::FixedPort) => Route::FixedPort,
        (Layout::SplitHolder, Layout::SplitHolder) => Route::Extended,
        (Layout::AliasDirect, Layout::AliasDirect) => Route::AliasedDirect,
        _ => Route::Existing,
    })
}

// Snapshot two flag lanes before the nullable old-value holder call, then read
// the third lane afterward. Every original coercion and call order is retained.
fn split_holder_frame(model:&Model<'_, '_>,site:FunctionSite,st:&[Statement<'_>])->Option<(VariantFrame,Layout)>{
 let [nonce,bytes,reseed,writer,flags,old,cursor,Statement::IfStatement(branch),combine,append,next]=st else{return None;};
 let seed=seed_declaration(model,nonce)?;
 let header=header_declaration(model,bytes,seed)?;
 let (seed_factory,reseed)=reseed_declaration(model,site,reseed,seed)?;
 let (writer,writer_factory)=writer_declaration(model,site,writer,seed,header)?;
 let v=one_variable(flags)?;
 let flag=plain_local(model,v)?;
 if model.binding_writes(flag)!=1{return None;}
 let (prefix,middle)=binary(v.init.as_ref()?,"|")?;
 let (low,zero)=binary(prefix,"|")?;
 let (middle,one)=binary(middle,"<<")?;
 if !number(zero,0.0)||!number(one,1.0){return None;}
 let low=captured_local(model,site,symbol(model,low)?)?;
 let middle=captured_local(model,site,symbol(model,middle)?)?;
 let (old,old_source)=alias_declaration(model,site,old,&[seed,header,writer,flag])?;
 let cursor=cursor_declaration(model,cursor,&[seed,header,writer,flag,old])?;
 let (test,null)=binary(&branch.test,"==")?;
 if symbol(model,test)!=Some(old)||!is_null(null){return None;}
 let tag=null_branch(model,header,seed,cursor,block(&branch.consequent)?)?;
 let [tuple,position]=block(branch.alternate.as_ref()?)? else{return None;};
 holder_tuple(model,tuple,writer,old,header)?;
 let position=assignment(expression_statement(position)?)?;
 if assigned_symbol(model,position)!=Some(cursor){return None;}
 holder_call(model,&position.right,writer,3.0)?;
 let combine=assignment(expression_statement(combine)?)?;
 let (prior,high)=binary(&combine.right,"|")?;
 let (high,two)=binary(high,"<<")?;
 if assigned_symbol(model,combine)!=Some(flag)||symbol(model,prior)!=Some(flag)||!number(two,2.0){return None;}
 let high=captured_local(model,site,symbol(model,high)?)?;
 let append=assignment(expression_statement(append)?)?;
 let (object,index)=member_target(&append.left)?;
 let (position,one)=binary(index,"+")?;
 let (value,stream)=binary(&append.right,"^")?;
 if symbol(model,object)!=Some(header)||symbol(model,position)!=Some(cursor)||!number(one,1.0)
   ||symbol(model,value)!=Some(flag)||!stream_call(model,stream,seed){return None;}
 if !distinct(&[seed,header,writer,flag,old,cursor,low,middle,high,seed_factory,writer_factory]){return None;}
 Some((VariantFrame{old:old_source,observer:next_frame(model,site,header,next)?,lanes:[low,middle,high],seed_factory,writer_factory,reseed,tag,fixed:false},Layout::SplitHolder))
}

fn lowered_next(model:&Model<'_, '_>,site:FunctionSite,header:Symbol,st:&[Statement<'_>])->Option<Symbol>{
 let [receiver,method,frame,set,emit]=st else{return None;};
 let receiver_v=one_variable(receiver)?;
 let receiver=plain_local(model,receiver_v)?;
 let observer=captured_local(model,site,symbol(model,receiver_v.init.as_ref()?)?)?;
 let method_v=one_variable(method)?;
 let method=plain_local(model,method_v)?;
 if symbol(model,member(method_v.init.as_ref()?,"next")?)!=Some(observer){return None;}
 let frame_v=one_variable(frame)?;
 let frame=plain_local(model,frame_v)?;
 if !matches!(frame_v.init.as_ref()?.without_parentheses(),Expression::ArrayExpression(a) if a.elements.is_empty())
   ||[receiver,method,frame].iter().any(|s|model.binding_writes(*s)!=0){return None;}
 let set=assignment(expression_statement(set)?)?;
 let (object,index)=member_target(&set.left)?;
 if symbol(model,object)!=Some(frame)||!number(index,1.0){return None;}
 let invoke=call(&set.right)?;
 let a=arity(invoke,4)?;
 if symbol(model,a[0])!=Some(frame)||!number(a[1],0.0)||symbol(model,a[2])!=Some(header)
   ||!matches!(a[3].without_parentheses(),Expression::ArrayExpression(a) if a.elements.is_empty()){return None;}
 let Expression::ArrowFunctionExpression(arrow)=invoke.callee.without_parentheses() else{return None;};
 if arrow.r#async||arrow.expression||arrow.params.rest.is_some()||arrow.params.items.len()!=4||!arrow.body.directives.is_empty(){return None;}
 let params:Vec<_>=arrow.params.items.iter().map(|p|{
   if p.initializer.is_some(){return None;}
   let BindingPattern::BindingIdentifier(id)=&p.pattern else{return None;};
   let id=id.symbol_id.get()?;
   (model.binding_writes(id)==0).then_some(id)
 }).collect::<Option<_>>()?;
 if !distinct(&params){return None;}
 let [write,Statement::ReturnStatement(ret)]=arrow.body.statements.as_slice() else{return None;};
 let write=assignment(expression_statement(write)?)?;
 let (object,index)=member_target(&write.left)?;
 if symbol(model,object)!=Some(params[0])||symbol(model,index)!=Some(params[1])
   ||symbol(model,&write.right)!=Some(params[3])||symbol(model,ret.argument.as_ref()?)!=Some(params[2]){return None;}
 let emit=call(expression_statement(emit)?)?;
 if symbol(model,member(&emit.callee,"call")?)!=Some(method){return None;}
 let a=arity(emit,2)?;
 if symbol(model,a[0])!=Some(receiver)||symbol(model,a[1])!=Some(frame)
   ||!distinct(&[receiver,method,frame,header,observer]){return None;}
 Some(observer)
}

fn fixed_port_frame(model:&Model<'_, '_>,site:FunctionSite,st:&[Statement<'_>])->Option<(VariantFrame,Layout)>{
 let [nonce,bytes,reseed,writer,flags,old,Statement::IfStatement(branch),tail @ ..]=st else{return None;};
 let seed=seed_declaration(model,nonce)?;
 let header=header_declaration(model,bytes,seed)?;
 let (seed_factory,reseed)=reseed_declaration(model,site,reseed,seed)?;
 let (writer,writer_factory)=pure_writer_declaration(model,site,writer,seed,header)?;
 let flags=assignment(expression_statement(flags)?)?;
 let (object,index)=member_target(&flags.left)?;
 let (flags,stream)=binary(&flags.right,"^")?;
 if symbol(model,object)!=Some(header)||!number(index,4.0)||!stream_call(model,stream,seed){return None;}
 let (low,middle,high)=captured_flags_expression(model,flags)?;
 for id in [low,middle,high]{captured_local(model,site,id)?;}
 let (old,old_source)=alias_declaration(model,site,old,&[seed,header,writer])?;
 let (test,null)=binary(&branch.test,"==")?;
 if symbol(model,test)!=Some(old)||!is_null(null){return None;}
 let tag=fixed_null_branch(model,header,seed,block(&branch.consequent)?)?;
 let [invoke]=block(branch.alternate.as_ref()?)? else{return None;};
 let invoke=call(expression_statement(invoke)?)?;
 let a=arity(invoke,3)?;
 if symbol(model,&invoke.callee)!=Some(writer)||symbol(model,a[0])!=Some(old)
  ||symbol(model,a[1])!=Some(header)||!number(a[2],4.0){return None;}
 if !distinct(&[seed,header,writer,old,low,middle,high,seed_factory,writer_factory]){return None;}
 Some((VariantFrame{old:old_source,observer:lowered_next(model,site,header,tail)?,lanes:[low,middle,high],seed_factory,writer_factory,reseed,tag,fixed:true},Layout::FixedPort))
}
