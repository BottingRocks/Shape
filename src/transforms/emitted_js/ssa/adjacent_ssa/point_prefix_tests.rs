#[test]
fn a_successful_current_store_can_prove_an_ordered_numeric_prefix() {
    let source="function f(input){let value={};value=+input;function make(){return 7;}function take(a,b){return [a,b];}let $SSA_0=make();return take(value|0,$SSA_0);}";
    let result = super::inline_thread_adjacent_ssa(source, &Default::default()).unwrap();
    assert_eq!(result.inlined_bindings, 1, "{}", result.code);
    // This pass consumes the stored version; dead-local cleanup removes
    // the now-uninitialized declaration in the driver.
    assert_eq!(result.code.matches("$SSA_0").count(), 1, "{}", result.code);
}
