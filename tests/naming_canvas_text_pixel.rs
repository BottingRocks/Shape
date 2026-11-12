//! Focused portable coverage for the seeded canvas-text pixel emitter.
//!
//! The owned family is one collector function: it creates a 96x96 canvas
//! through the captured document slot, schedules one text-sampling task queue
//! over it, registers the collector sink as `onEmpty`, starts the queue and
//! returns a teardown. The nested sampler owns a `willReadFrequently` `2d`
//! context of that same canvas, draws the caller glyph with `fillText`, reads
//! each caller point with `getImageData(..., 1, 1).data`, packs the RGBA lanes
//! and pushes the vector into the outer accumulator. The seeded tail derives a
//! `Math.random` 32-bit word, reseeds a captured hash with `word ^ constant` and
//! emits `[[], bytes]` on the captured observer slot.
//!
//! The portable positive corpus is the sample representative body wrapped in a
//! minimal carrier whose outer function must own more direct bindings than any
//! sibling helper stub; the queue factory is kept behind its source alias for
//! that reason. Variants cover the direct `context.fillText(...)` spelling,
//! the direct `queue.onEmpty(...)` spelling, truncated/untruncated random-word
//! placement, inline canvas creation and a renamed loop index. Negatives are
//! meaningful receiver, dataflow, ownership, aliasing and shadow changes: a
//! sampler-owned canvas, an overwritten initializer, reassigned
//! context/accumulator/observer/sink/queue aliases, a written formal, a
//! four-element array that merely mentions the random word and a direct-use
//! holder or push alias declared after its consuming call. The already-named
//! 5x5 `Image` canvas probe and a forwarding wrapper must not be claimed. The
//! existing canvas family module is not modified or replaced.
use shape::naming::name_threads;

const EVALUATOR: &str = "shared_canvas_text_pixel";
const NAME: &str = "emitSeededCanvasTextPixels";
const TARGET: &str = "$THREAD_8_94166";

// The sample body is nested in a real bootstrap that owns a broader direct
// binding set than any sibling helper. The carrier keeps the queue factory
// behind the source alias (`let $KEY_371 = $THREAD_7_112251;`) so `bootstrap`
// remains the unique broadest reachable main scope. A direct four-parameter
// helper stub otherwise ties the outer function at four direct bindings and
// `name_threads` refuses the program before any family proof can run.
const POSITIVE: &str = r##"function bootstrap(){
let $THREAD_7_112251 = function($a, $b, $c){ return { start: function(){}, onEmpty: function($f){}, empty: function(){} }; };
let $KEY_371 = $THREAD_7_112251;
let $KEY_95 = function($x){ return 0; };
let $KEY_231 = function($a, $b, $c, $d){ return 0; };
function $THREAD_8_94166($AP, $AQ, $AR, $AS) {
			let $KEY_4 = $AS;
			let $KEY_5 = $AP;
			let $KEY_6;
			let $KEY_2 = [];
			$KEY_6 = $AQ.createElement("canvas");
			let $KEY_0 = $KEY_371($AU => {
				let $AV = $KEY_2;
				let $AW = $AV.push;
				let $KEY_7 = $KEY_6;
				$KEY_7.height = 96;
				$KEY_7.width = 96;
				let $AX = [];
				try {
					let $AY = $KEY_7.getContext("2d", { willReadFrequently: true });
					$AY.font = $KEY_7.height - 2 + "px sans-serif";
					$AY.fillStyle = "#000";
					let $AZ = [$AY, $AY.fillText];
					$AZ[1].call($AZ[0], $AU[0], -1, $KEY_7.height);
					let i = 0;
					for (; i < $AU[1].length; i++) {
						let $BA = $AY.getImageData($AU[1][i][0], $AU[1][i][1], 1, 1).data;
						$AX.push($BA[0] << 24 | $BA[1] << 16 | $BA[2] << 8 | $BA[3]);
					}
					$AW.call($AV, $AX);
					let $BB = Math.random() * 4294967296;
					let $BC = [($BB | 0) >>> 24, ($BB | 0) >> 16 & 255, ($BB | 0) >> 8 & 255, ($BB | 0) & 255];
					$BB = $KEY_95(($BB | 0) ^ 127510030);
					let $BD = $KEY_2.length;
					let $BE = $KEY_231($BB, $BD, $BC, 3);
					for (;;) {
						--$BD;
						if (!($BD >= 0)) break;
						let $BF = $KEY_2[$BD];
						let $BG = $BF.length;
						$BE = $KEY_231($BB, $BG, $BC, $BE);
						for (;;) {
							--$BG;
							if (!($BG >= 0)) break;
							$BE = $KEY_231($BB, $BF[$BG], $BC, $BE);
							$BG = Number($BG);
						}
						$BD = Number($BD);
					}
					$KEY_5.next([[], $BC]);
					return void 0;
				} catch (e) {
					$AW.call($AV, $AX);
					let $BH = Math.random() * 4294967296;
					let $BI = [($BH | 0) >>> 24, ($BH | 0) >> 16 & 255, ($BH | 0) >> 8 & 255, ($BH | 0) & 255];
					$BH = $KEY_95(($BH | 0) ^ 127510030);
					let $BJ = $KEY_2.length;
					let $BK = $KEY_231($BH, $BJ, $BI, 3);
					for (;;) {
						--$BJ;
						if (!($BJ >= 0)) break;
						let $BL = $KEY_2[$BJ];
						let $BM = $BL.length;
						$BK = $KEY_231($BH, $BM, $BI, $BK);
						for (;;) {
							--$BM;
							if (!($BM >= 0)) break;
							$BK = $KEY_231($BH, $BL[$BM], $BI, $BK);
							$BM = Number($BM);
						}
						$BJ = Number($BJ);
					}
					$KEY_5.next([[], $BI]);
					return void 0;
				}
			}, 45, $AR);
			let $AT = [$KEY_0, $KEY_0.onEmpty];
			$AT[1].call($AT[0], () => {
				$KEY_4($KEY_2);
			});
			$KEY_0.start();
			function $THREAD_1_55686() {
				if ($KEY_0) {
					$KEY_0.empty();
				}
			}
			return $THREAD_1_55686;
		}
return $THREAD_8_94166;
}
bootstrap();"##;

const IMAGE_NEIGHBOR: &str = r##"function bootstrap(){
let $KEY_doc = document;
let $KEY_Image = Image;
let $KEY_ctx;
let $KEY_canvas;
let $KEY_img;
function $THREAD_7_5566($src, $cfg) {
function $THREAD_8_5567() {
try {
$KEY_ctx.drawImage($KEY_img, 0, 0);
try {
let $KEY_data = $KEY_ctx.getImageData(0, 0, 5, 5).data;
return void 0;
} catch (e) { return void 0; }
} catch (e_2) { return void 0; }
}
$KEY_canvas = $KEY_doc.createElement("canvas");
$KEY_canvas.width = 5;
$KEY_canvas.height = 5;
$KEY_ctx = $KEY_canvas.getContext("2d", { willReadFrequently: true });
$KEY_img = new $KEY_Image(5, 5);
$KEY_img.src = $src;
$KEY_img.onload = $THREAD_8_5567;
return $THREAD_8_5567;
}
return $THREAD_7_5566;
}
bootstrap();"##;

fn target_outcome<'a>(
    result: &'a shape::naming::NamedThreads,
    target: &str,
) -> &'a shape::naming::ThreadName {
    result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes"))
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        let outcome = target_outcome(&result, target);
        outcome.applied
            && outcome.proposed_name.as_deref() == Some(name)
            && outcome.evaluator.as_deref() == Some(EVALUATOR)
    })
}

fn our_claim(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        target_outcome(&result, target).evaluator.as_deref() == Some(EVALUATOR)
    })
}

fn parses(source: &str) -> bool {
    match name_threads(source, true) {
        Ok(_) => true,
        Err(error) => {
            eprintln!("name_threads error: {error}");
            false
        }
    }
}

fn denied(source: &str, target: &str) {
    assert!(parses(source), "fixture did not parse:\n{source}");
    assert!(!our_claim(source, target), "unexpected claim for:\n{source}");
    assert!(!named(source, target, NAME), "unexpected {NAME} for:\n{source}");
}

fn refuses(template: &str, target: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        denied(&template.replace(from, to), target);
    }
}

fn varied(template: &str, changes: &[(&str, &str)]) -> String {
    let mut source = template.to_string();
    for &(from, to) in changes {
        assert!(source.contains(from), "missing replacement {from}");
        source = source.replace(from, to);
    }
    source
}

/// The direct context method spelling: `ctx.fillText(glyph, -1, height)`.
fn direct_text() -> String {
    varied(
        POSITIVE,
        &[(
            "$AZ[1].call($AZ[0], $AU[0], -1, $KEY_7.height);",
            "$AY.fillText($AU[0], -1, $KEY_7.height);",
        )],
    )
}

/// The direct queue registration spelling: `queue.onEmpty(callback)`.
fn direct_on_empty() -> String {
    varied(
        POSITIVE,
        &[(
            "$AT[1].call($AT[0], () => {",
            "$KEY_0.onEmpty(() => {",
        )],
    )
}

/// The word is truncated once at its declaration instead of at each use site.
fn truncated_word() -> String {
    varied(
        POSITIVE,
        &[
            (
                "let $BB = Math.random() * 4294967296;",
                "let $BB = Math.random() * 4294967296 | 0;",
            ),
            (
                "let $BC = [($BB | 0) >>> 24, ($BB | 0) >> 16 & 255, ($BB | 0) >> 8 & 255, ($BB | 0) & 255];",
                "let $BC = [$BB >>> 24, $BB >> 16 & 255, $BB >> 8 & 255, $BB & 255];",
            ),
            (
                "$BB = $KEY_95(($BB | 0) ^ 127510030);",
                "$BB = $KEY_95($BB ^ 127510030);",
            ),
        ],
    )
}

/// The canvas is declared inline rather than pre-declared and assigned.
fn inline_canvas() -> String {
    varied(
        POSITIVE,
        &[
            ("let $KEY_6;", ""),
            (
                "$KEY_6 = $AQ.createElement(\"canvas\");",
                "let $KEY_6 = $AQ.createElement(\"canvas\");",
            ),
        ],
    )
}

/// `for (let j...)` with a renamed loop index instead of the shared `i`.
fn renamed_loop_index() -> String {
    varied(
        POSITIVE,
        &[
            ("let i = 0;", "let j = 0;"),
            (
                "for (; i < $AU[1].length; i++) {",
                "for (; j < $AU[1].length; j++) {",
            ),
            (
                "$AU[1][i][0], $AU[1][i][1]",
                "$AU[1][j][0], $AU[1][j][1]",
            ),
        ],
    )
}

fn alpha() -> String {
    varied(
        POSITIVE,
        &[
            ("$THREAD_1_55686", "$THREAD_901_888_3"),
            ("$THREAD_8_94166", "$THREAD_900_777_2"),
            ("$THREAD_7_112251", "$A_251"),
            ("$KEY_371", "$A_371"),
            ("$KEY_95", "$A_95"),
            ("$KEY_231", "$A_231"),
            ("$AP", "$A_1"),
            ("$AQ", "$A_2"),
            ("$AR", "$A_3"),
            ("$AS", "$A_4"),
        ],
    )
}

#[test]
fn witness_body_is_named() {
    assert!(named(POSITIVE, TARGET, NAME), "{POSITIVE}");
    assert!(our_claim(POSITIVE, TARGET));
}

#[test]
fn lexical_and_declaration_variants_are_claimed() {
    for (source, target) in [
        (direct_text(), TARGET.to_string()),
        (direct_on_empty(), TARGET.to_string()),
        (truncated_word(), TARGET.to_string()),
        (inline_canvas(), TARGET.to_string()),
        (renamed_loop_index(), TARGET.to_string()),
        (alpha(), "$THREAD_900_777_2".to_string()),
    ] {
        assert!(named(&source, &target, NAME), "did not name:\n{source}");
        assert!(our_claim(&source, &target));
    }
}

#[test]
fn changed_canvas_context_and_text_are_refused() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            ("$KEY_7.height = 96;", "$KEY_7.height = 95;"),
            ("$KEY_7.width = 96;", "$KEY_7.width = 95;"),
            (
                "let $AY = $KEY_7.getContext(\"2d\", { willReadFrequently: true });",
                "let $AY = $KEY_OTHER.getContext(\"2d\", { willReadFrequently: true });",
            ),
            (
                "let $AY = $KEY_7.getContext(\"2d\", { willReadFrequently: true });",
                "let $AY = $KEY_7.getContext(\"2d\", { willReadFrequently: false });",
            ),
            ("$AY.font = $KEY_7.height - 2 + \"px sans-serif\";", "$AY.font = $KEY_7.height - 2 + \"px serif\";"),
            ("$AY.fillStyle = \"#000\";", "$AY.fillStyle = \"#fff\";"),
            ("let $AZ = [$AY, $AY.fillText];", "let $AZ = [$AY, $AY.fillRect];"),
            (
                "$AZ[1].call($AZ[0], $AU[0], -1, $KEY_7.height);",
                "$AZ[1].call($AZ[0], $KEY_0[0], -1, $KEY_7.height);",
            ),
        ],
    );
}

#[test]
fn changed_pixel_receiver_geometry_and_packing_are_refused() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            (
                "let $BA = $AY.getImageData($AU[1][i][0], $AU[1][i][1], 1, 1).data;",
                "let $BA = $KEY_7.getImageData($AU[1][i][0], $AU[1][i][1], 1, 1).data;",
            ),
            (
                "let $BA = $AY.getImageData($AU[1][i][0], $AU[1][i][1], 1, 1).data;",
                "let $BA = $AY.getImageData($AU[1][i][0], $AU[1][i][1], 5, 5).data;",
            ),
            (
                "let $BA = $AY.getImageData($AU[1][i][0], $AU[1][i][1], 1, 1).data;",
                "let $BA = $AY.getImageData($AU[1][i][0], $AU[1][i][1], 1, 2).data;",
            ),
            (
                "$AX.push($BA[0] << 24 | $BA[1] << 16 | $BA[2] << 8 | $BA[3]);",
                "$AX.push($BA[0] << 8 | $BA[1] << 16 | $BA[2] << 24 | $BA[3]);",
            ),
            (
                "$AW.call($AV, $AX);",
                "$AW.call($AV, $BK);",
            ),
        ],
    );
}

#[test]
fn changed_seeded_sink_and_observer_are_refused() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            ("Math.random() * 4294967296;", "1;"),
            ("* 4294967296;", "* 4294967295;"),
            ("$KEY_95((", "$KEY_96(("),
            ("^ 127510030", "+ 127510030"),
            ("$KEY_5.next([[], $", "$KEY_2.next([[], $"),
            ("next([[], $", "next([[$KEY_2], $"),
            ("$KEY_4($KEY_2);", "$KEY_4($AX);"),
            ("$KEY_0.empty();", "$KEY_2.empty();"),
            ("$KEY_0.start();", "$KEY_2.start();"),
        ],
    );
}

#[test]
fn changed_ownership_and_forwarding_wrappers_are_refused() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            (
                "let $KEY_0 = $KEY_371($AU => {",
                "let $KEY_0 = $AP($AU => {",
            ),
            (
                "let $KEY_0 = $KEY_371($AU => {",
                "let $KEY_0 = createQueue($AU => {",
            ),
            (
                "$AW.call($AV, $AX);",
                "$KEY_95.call($AV, $AX);",
            ),
        ],
    );
    // A wrapper that merely forwards to the proved collector is not the family.
    let wrapper = POSITIVE.replace(
        "function $THREAD_8_94166(",
        "function $THREAD_5_9001() { return $THREAD_8_94166; }\nfunction $THREAD_8_94166(",
    );
    assert!(parses(&wrapper));
    assert!(!our_claim(&wrapper, "$THREAD_5_9001"));
    // The independently proved collector is still claimed.
    assert!(named(&wrapper, TARGET, NAME));
}

#[test]
fn changed_ownership_and_aliasing_are_refused() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            // The canvas may not be created inside the nested sampler.
            (
                "let $KEY_7 = $KEY_6;",
                "let $KEY_7 = $AQ.createElement(\"canvas\");",
            ),
            // An initializer overwritten by a later write is not the value.
            (
                "let $KEY_7 = $KEY_6;",
                "let $KEY_7 = $KEY_6; $KEY_7 = $AQ.createElement(\"canvas\");",
            ),
            // A context alias written after its initializer is not stable.
            (
                "let $AY = $KEY_7.getContext(\"2d\", { willReadFrequently: true });",
                "let $AY = $KEY_7.getContext(\"2d\", { willReadFrequently: true }); $AY = $AQ.createElement(\"canvas\").getContext(\"2d\", { willReadFrequently: true });",
            ),
            // The accumulator alias may not be reassigned to a fresh array.
            ("let $AW = $AV.push;", "$AV = []; let $AW = $AV.push;"),
            // The observer and sink copies must still denote their slots.
            ("let $KEY_5 = $AP;", "let $KEY_5 = $AP; $KEY_5 = $AQ;"),
            ("let $KEY_4 = $AS;", "let $KEY_4 = $AS; $KEY_4 = $AP;"),
            // A written formal no longer denotes its incoming argument.
            (
                "function $THREAD_8_94166($AP, $AQ, $AR, $AS) {",
                "function $THREAD_8_94166($AP, $AQ, $AR, $AS) { $AS = $AP;",
            ),
            // The queue is defined once and never rewritten.
            ("$KEY_0.start();", "$KEY_0 = $KEY_2; $KEY_0.start();"),
        ],
    );
}

#[test]
fn direct_use_aliases_defined_after_their_calls_are_refused() {
    // Each direct-use holder and push alias is consumed by symbol identity, so
    // the alias origin must still precede the call. Moving the single
    // declaration below the call it feeds reads the binding in its temporal
    // dead zone and the collector cannot execute.
    //
    // The `[ctx, ctx.fillText]` holder.
    let text_holder = varied(
        POSITIVE,
        &[(
            "\t\t\t\t\tlet $AZ = [$AY, $AY.fillText];\n\t\t\t\t\t$AZ[1].call($AZ[0], $AU[0], -1, $KEY_7.height);",
            "\t\t\t\t\t$AZ[1].call($AZ[0], $AU[0], -1, $KEY_7.height);\n\t\t\t\t\tlet $AZ = [$AY, $AY.fillText];",
        )],
    );
    denied(&text_holder, TARGET);

    // The `[queue, queue.onEmpty]` holder.
    let queue_holder = varied(
        POSITIVE,
        &[(
            "\t\t\tlet $AT = [$KEY_0, $KEY_0.onEmpty];\n\t\t\t$AT[1].call($AT[0], () => {\n\t\t\t\t$KEY_4($KEY_2);\n\t\t\t});",
            "\t\t\t$AT[1].call($AT[0], () => {\n\t\t\t\t$KEY_4($KEY_2);\n\t\t\t});\n\t\t\tlet $AT = [$KEY_0, $KEY_0.onEmpty];",
        )],
    );
    denied(&queue_holder, TARGET);

    // The push alias. Both sampler arms (try and catch) consume it, so the
    // declaration must precede the call in each arm: move it below the catch
    // arm's call, leaving every consuming call before the single origin.
    let push_alias = varied(
        POSITIVE,
        &[
            ("\t\t\t\tlet $AW = $AV.push;\n", ""),
            (
                "\t\t\t\t\t$AW.call($AV, $AX);\n\t\t\t\t\tlet $BH = Math.random() * 4294967296;",
                "\t\t\t\t\t$AW.call($AV, $AX);\n\t\t\t\t\tlet $AW = $AV.push;\n\t\t\t\t\tlet $BH = Math.random() * 4294967296;",
            ),
        ],
    );
    denied(&push_alias, TARGET);
}

#[test]
fn mention_only_byte_array_is_refused() {
    // A four-element array that merely mentions the random word is not a
    // four-lane RGBA byte derivation. Both emission branches carry the tail.
    let source = varied(
        POSITIVE,
        &[
            (
                "[($BB | 0) >>> 24, ($BB | 0) >> 16 & 255, ($BB | 0) >> 8 & 255, ($BB | 0) & 255]",
                "[$BB, $BB, $BB, $BB]",
            ),
            (
                "[($BH | 0) >>> 24, ($BH | 0) >> 16 & 255, ($BH | 0) >> 8 & 255, ($BH | 0) & 255]",
                "[$BH, $BH, $BH, $BH]",
            ),
        ],
    );
    denied(&source, TARGET);
}

#[test]
fn image_canvas_probe_neighbor_is_not_claimed() {
    denied(IMAGE_NEIGHBOR, "$THREAD_7_5566");
}

#[test]
fn human_name_and_existing_edits_are_preserved() {
    let human = POSITIVE.replace("$THREAD_8_94166", "collectCanvasTextSignals");
    let human_result = name_threads(&human, true).unwrap();
    assert!(
        human_result
            .source
            .contains("function collectCanvasTextSignals(")
    );
    assert!(!human_result
        .source
        .contains("function emitSeededCanvasTextPixels("));

    let result = name_threads(POSITIVE, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(NAME)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .collect();
    assert_eq!(applied.len(), 1, "expected exactly one applied target");
    let mut replay = POSITIVE.to_string();
    let mut edits: Vec<_> = result.report.edits.iter().collect();
    edits.sort_by(|left, right| right.start.cmp(&left.start).then(right.end.cmp(&left.end)));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}

/// A refusal must concern the original named candidate in a parse-valid
/// program; a parser failure or a missing target is never a passing negative.
fn iso_denied(source: &str, target: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(parsed.errors.is_empty(), "parser errors: {:?}\n{source}", parsed.errors);
    let result = name_threads(source, true).expect("mutant must analyze");
    let outcome = target_outcome(&result, target);
    assert_ne!(outcome.evaluator.as_deref(), Some(EVALUATOR), "candidate borrowed a proof:\n{source}");
    assert!(!(outcome.applied && outcome.proposed_name.as_deref() == Some(NAME)), "unsupported role applied:\n{source}");
}

/// Mutate whole mixer regions, leaving each nonce/header/reseed/next intact.
/// Both arms must be changed for the original existential matcher to fail.
fn iso_mixer_regions(template: &str, success: bool, catch: bool, nested: bool) -> String {
    let mut source = template.to_owned();
    for (enabled, count, bytes, donor) in [
        (success, "$BD", "$BC", "neverMixSuccess"),
        (catch, "$BJ", "$BI", "neverMixCatch"),
    ] {
        if !enabled { continue; }
        let start = source.find(&format!("let {count} = $KEY_2.length;")).expect("mixer start");
        let end = start + source[start..].find(&format!("$KEY_5.next([[], {bytes}]);")).expect("same arm emission");
        let replacement = if nested {
            format!("function {donor}() {{\n{}\n}}\n", &source[start..end])
        } else {
            String::new()
        };
        source.replace_range(start..end, &replacement);
    }
    source
}

#[test]
fn mixer_flow_nonce_only_emission_rejects_the_same_candidate() {
    // Delete both contiguous mixer regions to isolate nonce-only emission.
    iso_denied(&iso_mixer_regions(POSITIVE, true, true, false), TARGET);
    // Neither alternate arm may rescue an incomplete emission path.
    iso_denied(&iso_mixer_regions(POSITIVE, true, false, false), TARGET);
    iso_denied(&iso_mixer_regions(POSITIVE, false, true, false), TARGET);
    iso_denied(&iso_mixer_regions(&alpha(), true, true, false), "$THREAD_900_777_2");
}

#[test]
fn mixer_flow_unrelated_vector_cannot_supply_mixer_values() {
    let source = varied(POSITIVE, &[
        ("let $KEY_2 = [];", "let $KEY_2 = []; let unrelated = [[7]];"),
        ("= $KEY_2.length;", "= unrelated.length;"),
        ("= $KEY_2[$BD];", "= unrelated[$BD];"),
        ("= $KEY_2[$BJ];", "= unrelated[$BJ];"),
    ]);
    iso_denied(&source, TARGET);
    // Preserve the genuine accumulator length but encode unrelated rows.
    let wrong_rows = source.replace("= unrelated.length;", "= $KEY_2.length;");
    iso_denied(&wrong_rows, TARGET);
}

#[test]
fn mixer_flow_nested_and_sibling_donors_cannot_restore_encoding() {
    iso_denied(&iso_mixer_regions(POSITIVE, true, true, true), TARGET);
    // An intact sibling collector contains every witness, but none belongs to
    // the candidate whose own two mixer regions have been removed.
    let donor = POSITIVE[POSITIVE.find("function $THREAD_8_94166(").unwrap()
        ..POSITIVE.rfind("return $THREAD_8_94166;").unwrap()]
        .replace(TARGET, "$THREAD_99_94166");
    let mutant = iso_mixer_regions(POSITIVE, true, true, false)
        .replace("return $THREAD_8_94166;", &format!("{donor}\nreturn $THREAD_8_94166;"));
    iso_denied(&mutant, TARGET);
    assert!(named(&mutant, "$THREAD_99_94166", NAME), "independent donor must still qualify");
}

#[test]
fn mixer_flow_mixer_header_state_cursor_and_value_edges_are_required() {
    for changes in [
        vec![("$KEY_231($BB,", "$KEY_231(0,"), ("$KEY_231($BH,", "$KEY_231(0,")],
        vec![("$KEY_231($BB, $BF[$BG],", "$KEY_231($BB, 7,"), ("$KEY_231($BH, $BL[$BM],", "$KEY_231($BH, 7,")],
        vec![("$BC, $BE)", "$BC, 3)"), ("$BI, $BK)", "$BI, 3)")],
        vec![("$BC, 3)", "[], 3)"), ("$BI, 3)", "[], 3)")],
        vec![("$BB, $BG, $BC", "$BB, $BG, []"), ("$BH, $BM, $BI", "$BH, $BM, []")],
        vec![("$BB, $BF[$BG], $BC", "$BB, $BF[$BG], []"), ("$BH, $BL[$BM], $BI", "$BH, $BL[$BM], []")],
        vec![("$BE = $KEY_231", "$BE = $KEY_95"), ("$BK = $KEY_231", "$BK = $KEY_95")],
        vec![("if (!($BD >= 0)) break;", "if (true) break;"), ("if (!($BJ >= 0)) break;", "if (true) break;")],
    ] {
        iso_denied(&varied(POSITIVE, &changes), TARGET);
    }
}

#[test]
fn mixer_flow_mixer_cannot_alias_the_reseed_factory() {
    let mixer = "let $KEY_231 = function($a, $b, $c, $d){ return 0; };";
    let distinct = POSITIVE.replace(mixer,
        "let mixerAlias = function($a, $b, $c, $d){ return 0; }; let $KEY_231 = mixerAlias;");
    assert!(named(&distinct, TARGET, NAME), "distinct helper alias must qualify");
    for alias in [
        "let $KEY_231 = $KEY_95;",
        "let reseedAlias = $KEY_95; let $KEY_231 = reseedAlias;",
    ] {
        iso_denied(&POSITIVE.replace(mixer, alias), TARGET);
    }
}

#[test]
fn rgba_tree_complete_rgba_expression_rejects_additional_leaves() {
    let packing = "$BA[0] << 24 | $BA[1] << 16 | $BA[2] << 8 | $BA[3]";
    iso_denied(&varied(POSITIVE, &[(packing, &format!("{packing} | 1"))]), TARGET);
    for extra in ["1", "unknownLane", "$BA[4]", "$BA[0] << 24", "($BA[3] ^ 1)", "0"] {
        // The `| 1` mutation adds an unrecognized leaf. The other mutations retain all four
        // recognized lanes while adding unknown or duplicate leaves. Duplicate lanes and `| 0` are
        // conservative unsupported shapes, not claims of semantic corruption.
        let source = varied(POSITIVE, &[(packing, &format!("({packing}) | {extra}"))]);
        iso_denied(&source, TARGET);
    }
    let source = varied(&alpha(), &[(packing, &format!("1 | ({packing})"))]);
    iso_denied(&source, "$THREAD_900_777_2");
}

#[test]
fn rgba_tree_parenthesized_exact_rgba_tree_remains_supported() {
    let source = varied(POSITIVE, &[
        ("$BA[0] << 24 | $BA[1] << 16 | $BA[2] << 8 | $BA[3]",
         "(($BA[0] << 24) | ($BA[1] << 16)) | (($BA[2] << 8) | $BA[3])"),
    ]);
    assert!(named(&source, TARGET, NAME), "supported packing:\n{source}");
}

#[test]
fn pixel_alias_vector_clear_rejects_the_same_candidate() {
    let append = "$AW.call($AV, $AX);";
    let clear = "$AX.length = 0; $AW.call($AV, $AX);";
    assert_eq!(POSITIVE.matches(append).count(), 2);
    // Reset both branches together, then exercise each branch alone.
    iso_denied(&POSITIVE.replace(append, clear), TARGET);
    iso_denied(&POSITIVE.replacen(append, clear, 1), TARGET);
    let catch = POSITIVE.rfind(append).expect("catch append");
    let mut catch_only = POSITIVE.to_string();
    catch_only.replace_range(catch..catch + append.len(), clear);
    iso_denied(&catch_only, TARGET);
    iso_denied(&alpha().replace(append, clear), "$THREAD_900_777_2");
}

#[test]
fn pixel_alias_alias_mutations_and_vector_escapes_are_refused() {
    let append = "$AW.call($AV, $AX);";
    for replacement in [
        "let rowAlias = $AX; rowAlias.length = 0; $AW.call($AV, $AX);",
        "let rowAlias = $AX; let rowAlias2 = rowAlias; rowAlias2[0] = 7; $AW.call($AV, $AX);",
        "$AX[0] = 7; $AW.call($AV, $AX);",
        "$AX = [7]; $AW.call($AV, $AX);",
        "delete $AX[0]; $AW.call($AV, $AX);",
        "$AX.splice(0); $AW.call($AV, $AX);",
        "mutateRow($AX); $AW.call($AV, $AX);",
        "let rowAlias = $AX; mutateRow(rowAlias); $AW.call($AV, $AX);",
    ] {
        iso_denied(&POSITIVE.replace(append, replacement), TARGET);
    }
    // A mutation inside the fill loop must not borrow the preceding push.
    let push = "$AX.push($BA[0] << 24 | $BA[1] << 16 | $BA[2] << 8 | $BA[3]);";
    iso_denied(&POSITIVE.replace(push, &format!("{push} $AX.length = 0;")), TARGET);
    // A property write through an alias declared before sampling also counts.
    let alias = POSITIVE.replace("let $AX = [];", "let $AX = []; let rowAlias = $AX;");
    iso_denied(&alias.replace(append, "rowAlias.length = 0; $AW.call($AV, $AX);"), TARGET);
}

#[test]
fn pixel_alias_stable_pixel_alias_and_direct_appends_remain_supported() {
    let alias = varied(POSITIVE, &[
        ("let $AX = [];", "let $AX = []; let rowAlias = $AX; let rowAlias2 = rowAlias;"),
        ("$AX.push(", "rowAlias2.push("),
        ("$AW.call($AV, $AX);", "$AW.call($AV, rowAlias2);"),
    ]);
    assert!(named(&alias, TARGET, NAME), "stable pixel alias:\n{alias}");
    let direct = POSITIVE.replace("$AW.call($AV, $AX);", "$AV.push($AX);");
    assert!(named(&direct, TARGET, NAME), "direct append:\n{direct}");
    for append in [
        "let wrongReceiver = []; wrongReceiver.push($AX);",
        "$AV.push(...$AX);",
        "$AV.push($AX, ...[]);",
    ] {
        iso_denied(&POSITIVE.replace("$AW.call($AV, $AX);", append), TARGET);
    }
}

fn cached_named(source: &str, target: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(parsed.errors.is_empty(), "parser errors: {:?}", parsed.errors);
    let result = name_threads(source, true).expect("cached collector must analyze");
    let outcome = target_outcome(&result, target);
    assert_eq!(outcome.evaluator.as_deref(), Some(EVALUATOR), "exact collector lost proof");
    assert_eq!(outcome.proposed_name.as_deref(), Some(NAME));
    assert!(outcome.applied, "exact collector name not applied");
}

#[test]
fn forward_while_forward_while_forward_for_is_named() {
    cached_named(FORWARD_WHILE, "$THREAD_0_33874");
}

#[test]
fn forward_for_forward_while_forward_for_is_named() {
    cached_named(FORWARD_FOR, "$THREAD_5_87153");
}

#[test]
fn reverse_outer_reverse_outer_forward_inner_is_named() {
    cached_named(REVERSE_OUTER, "$THREAD_5_56405");
}

#[test]
fn late_setup_reverse_reverse_encoding_with_late_captured_setup_is_named() {
    cached_named(LATE_SETUP_REVERSE, "$THREAD_3_77917");
}

#[test]
fn late_setup_reverse_late_helper_initializers_and_ordered_aliases_are_named() {
    // Both forms initialize the captures before the collector can be called.
    // The helper identity reader must not impose the nested body's position
    // on these enclosing setup statements.
    let initialized = varied(LATE_SETUP_REVERSE, &[
        ("let $KEY_52, $KEY_152, $KEY_190;", "let $KEY_152;"),
        ("$KEY_52 = numericWriter;", "let $KEY_52 = numericWriter;"),
        ("$KEY_190 = reseedFactory;", "let $KEY_190 = reseedFactory;"),
    ]);
    cached_named(&initialized, "$THREAD_3_77917");
    let aliased = varied(LATE_SETUP_REVERSE, &[
        ("$KEY_52 = numericWriter;",
         "let writerAlias = numericWriter; let writerAlias2 = writerAlias; $KEY_52 = writerAlias2;"),
        ("$KEY_190 = reseedFactory;",
         "let hashAlias = reseedFactory; $KEY_190 = hashAlias;"),
        ("$KEY_52", "$KEY_7052"),
        ("$KEY_190", "$KEY_7190"),
        ("$THREAD_3_77917", "$THREAD_72_105"),
    ]);
    cached_named(&aliased, "$THREAD_72_105");
}

#[test]
fn cached_forward_and_mixed_encoding_allow_late_captured_setup() {
    for (template, target, hash, mixer) in [
        (FORWARD_WHILE, "$THREAD_0_33874", "$KEY_286", "$KEY_58"),
        (REVERSE_OUTER, "$THREAD_5_56405", "$KEY_67", "$KEY_291"),
    ] {
        let hash_setup = format!("let {hash} = function(x){{ return 0; }};");
        let mixer_setup = format!("let {mixer} = function(a,b,c,d){{ return 0; }};");
        let returned = format!("return {target};");
        let late_setup = format!("{hash_setup}\n{mixer_setup}\n{returned}");
        let source = varied(template, &[
            (&hash_setup, ""), (&mixer_setup, ""), (&returned, &late_setup),
        ]);
        cached_named(&source, target);
    }
}

#[test]
fn late_setup_reverse_late_helpers_cannot_share_or_overwrite_origins() {
    for setup in [
        "$KEY_52 = reseedFactory;",
        "let hashAlias = reseedFactory; $KEY_52 = hashAlias;",
        "let hashAlias = reseedFactory; let hashAlias2 = hashAlias; $KEY_52 = hashAlias2;",
        "$KEY_52 = numericWriter; $KEY_52 = reseedFactory;",
        "let writerAlias = numericWriter; writerAlias = reseedFactory; $KEY_52 = writerAlias;",
        // Later alias initializers are immediate reads, unlike closure bodies.
        "$KEY_52 = writerAlias; let writerAlias = numericWriter;",
        "let writerAlias = writerAlias2; let writerAlias2 = numericWriter; $KEY_52 = writerAlias;",
        concat!(
            "let writerAlias0 = numericWriter; let writerAlias1 = writerAlias0; ",
            "let writerAlias2 = writerAlias1; let writerAlias3 = writerAlias2; ",
            "let writerAlias4 = writerAlias3; let writerAlias5 = writerAlias4; ",
            "let writerAlias6 = writerAlias5; let writerAlias7 = writerAlias6; ",
            "let writerAlias8 = writerAlias7; $KEY_52 = writerAlias8;",
        ),
    ] {
        iso_denied(&varied(LATE_SETUP_REVERSE, &[("$KEY_52 = numericWriter;", setup)]), "$THREAD_3_77917");
    }
    // A distinct capture can still resolve through the reseeder's late alias.
    let same = varied(LATE_SETUP_REVERSE, &[
        ("$KEY_52 = numericWriter;\n$KEY_190 = reseedFactory;",
         "$KEY_190 = reseedFactory; $KEY_52 = $KEY_190;"),
    ]);
    iso_denied(&same, "$THREAD_3_77917");
    // Initializer origins overwritten later must also be refused.
    let overwritten = varied(LATE_SETUP_REVERSE, &[
        ("let $KEY_52, $KEY_152, $KEY_190;", "let $KEY_52 = numericWriter; let $KEY_152, $KEY_190;"),
        ("$KEY_52 = numericWriter;", "$KEY_52 = reseedFactory;"),
    ]);
    iso_denied(&overwritten, "$THREAD_3_77917");
}

#[test]
fn late_setup_reverse_nested_setters_cannot_donate_helper_definitions() {
    for (before, after) in [
        ("$KEY_52 = numericWriter;", "function neverSetWriter() { $KEY_52 = numericWriter; }"),
        ("$KEY_190 = reseedFactory;", "let neverSetHash = () => { $KEY_190 = reseedFactory; };"),
        ("$KEY_52 = numericWriter;",
         "let writerAlias; function neverSetAlias() { writerAlias = numericWriter; } $KEY_52 = writerAlias;"),
    ] {
        iso_denied(&varied(LATE_SETUP_REVERSE, &[(before, after)]), "$THREAD_3_77917");
    }
}

#[test]
fn late_setup_reverse_reverse_encoding_still_requires_each_ordered_edge() {
    // Mutate success alone, then catch alone. Neither arm can donate its proof.
    for (before, after) in [
        ("$BZ.call($BY, $CA);", "$CA.length = 0; $BZ.call($BY, $CA);"),
        ("let $BZ = $KEY_2.push;", "let wrong = []; let $BZ = wrong.push;"),
        ("--$CG;", "$CG -= 2;"),
        ("--$CJ;", "$CJ -= 2;"),
        ("$CJ = Number($CJ);", "$CJ = Number($CG);"),
        ("$CG = Number($CG);", "$CG = Number($CJ);"),
        ("let $CI = $KEY_2[$CG];", "let $CI = $CA[$CG];"),
        ("$CH = $KEY_52($CE, $CI[$CJ], $CF, $CH);", "$CH = $KEY_52($CE, $CI[$CJ], $CF, 3);"),
        ("$CH = $KEY_52($CE, $CJ, $CF, $CH);", "$CH = $KEY_52($CE, $CJ, [], $CH);"),
        ("--$CM;", "$CM -= 2;"),
        ("--$CP;", "$CP -= 2;"),
        ("$CP = Number($CP);", "$CP = Number($CM);"),
        ("$CN = $KEY_52($CK, $CO[$CP], $CL, $CN);", "$CN = $KEY_52($CK, $CO[$CP], $CL, 3);"),
        ("$KEY_4.next([[], $CF]);", "$KEY_4.next([[], []]);"),
    ] {
        iso_denied(&varied(LATE_SETUP_REVERSE, &[(before, after)]), "$THREAD_3_77917");
    }
    // The normalization must follow this counter's decrement and use.
    let reordered = varied(LATE_SETUP_REVERSE, &[
        ("\t\t\t\t\t\t\t$CJ = Number($CJ);\n", ""),
        ("\t\t\t\t\t\t\t--$CJ;", "\t\t\t\t\t\t\t$CJ = Number($CJ);\n\t\t\t\t\t\t\t--$CJ;"),
    ]);
    iso_denied(&reordered, "$THREAD_3_77917");
    let shadowed = varied(LATE_SETUP_REVERSE, &[
        ("let $KEY_52, $KEY_152, $KEY_190;",
         "let $KEY_52, $KEY_152, $KEY_190; let Number = x => x + 1;"),
    ]);
    iso_denied(&shadowed, "$THREAD_3_77917");
}

#[test]
fn cached_forward_encoding_refuses_broken_lineage_and_skipped_indices() {
    let target = "$THREAD_0_33874";
    // Each change affects success alone. An intact catch cannot donate proof.
    for (before, after) in [
        ("$CG = $KEY_58($CC, $CH[j], $CD, $CG);", "$CG = $KEY_58($CC, $CH[j], $CD, 3);"),
        ("$CG = $KEY_58($CC, $CI, $CD, $CG);", "$CG = $KEY_58($CC, $CI, $CD, 3);"),
        ("$CG = $KEY_58($CC, $CH[j], $CD, $CG);", "$CG = $KEY_58($CC, $CH[j], [], $CG);"),
        ("$CG = $KEY_58($CC, $CH[j], $CD, $CG);", "$CG = $KEY_58(0, $CH[j], $CD, $CG);"),
        ("$CG = $KEY_58($CC, $CH[j], $CD, $CG);", "$CG = $KEY_286($CC, $CH[j], $CD, $CG);"),
        ("let $CH = $KEY_4[$CF];", "let $CH = $BZ[$CF];"),
        ("let $CE = $KEY_4.length;", "let $CE = $BZ.length;"),
        ("let $CG = $KEY_58($CC, $CE, $CD, 3);", "let $CG = $KEY_58($CC, $CF, $CD, 3);"),
        ("let $CF = 0;", "let $CF = 1;"),
        ("while ($CF < $CE)", "while ($CF <= $CE)"),
        ("++$CF;", "$CF += 2;"),
        ("let j = 0;", "let j = 1;"),
        ("for (; j < $CI; j++)", "for (; j <= $CI; j++)"),
        ("for (; j < $CI; j++)", "for (; j < $CI; j += 2)"),
        ("$CH[j]", "$CH[$CF]"),
        ("++$CF;", "++$CF; break;"),
        ("let $CF = 0;", "let $CF = 0; $CF = 0;"),
        ("let $CF = 0;", "let $CF = 0; $BZ.length = 0;"),
        ("$BY.call($BX, $BZ);", "$BZ.length = 0; $BY.call($BX, $BZ);"),
    ] {
        iso_denied(&varied(FORWARD_WHILE, &[(before, after)]), target);
    }
    // A broken catch must also refuse the same candidate.
    iso_denied(&varied(FORWARD_WHILE, &[("let $CM = 0;", "let $CM = 1;")]), target);
    // Definition order is part of the bounded forward shape.
    let swapped = varied(FORWARD_WHILE, &[(
        "let $CE = $KEY_4.length;\n\t\t\t\t\tlet $CF = 0;",
        "let $CF = 0;\n\t\t\t\t\tlet $CE = $KEY_4.length;",
    )]);
    iso_denied(&swapped, target);
}

#[test]
fn cached_forward_encoding_cannot_borrow_nested_or_sibling_donors() {
    let start = FORWARD_WHILE.find("while ($CF < $CE)").expect("outer loop");
    let end = FORWARD_WHILE[start..].find("$KEY_5.next([[], $CD]);").unwrap() + start;
    let region = &FORWARD_WHILE[start..end];
    for donor in [
        format!("function neverCalled() {{ {region} }}"),
        format!("if (false) {{ {region} }}"),
        format!("let donor = () => {{ {region} }};"),
    ] {
        let source = FORWARD_WHILE.replacen(region, &donor, 1);
        iso_denied(&source, "$THREAD_0_33874");
    }
    // A complete sibling collector cannot repair the broken exact target.
    let broken = varied(FORWARD_WHILE, &[("let $CF = 0;", "let $CF = 1;")]);
    let collector_start = FORWARD_WHILE.find("function $THREAD_0_33874(").unwrap();
    let collector_end = FORWARD_WHILE.rfind("return $THREAD_0_33874;").unwrap();
    let sibling = FORWARD_WHILE[collector_start..collector_end]
        .replace("$THREAD_0_33874", "$THREAD_901_777");
    let source = broken.replace("return $THREAD_0_33874;",
        &format!("{sibling}\nreturn [$THREAD_0_33874, $THREAD_901_777];"));
    iso_denied(&source, "$THREAD_0_33874");
}

#[test]
fn cached_mixed_encoding_refuses_broken_inner_loop_and_catch() {
    for (before, after) in [
        ("let j = 0;", "let j = 1;"),
        ("for (; j < $CM; j++)", "for (; j < $CM; j += 2)"),
        ("$CL[j]", "$CL[$CJ]"),
        ("$CK = $KEY_291($CH, $CL[j], $CI, $CK);", "$CK = $KEY_291($CH, $CL[j], $CI, 3);"),
        ("let k = 0;", "let k = 1;"),
    ] {
        iso_denied(&varied(REVERSE_OUTER, &[(before, after)]), "$THREAD_5_56405");
    }
}

// Cached collector bodies are byte-exact slices, with only inert captured
// helper bindings added in a faithful owning bootstrap. No giant seed needed.
const FORWARD_WHILE: &str = r##"function bootstrap(){
let queueFactory = function(a,b,c){ return {start:function(){},onEmpty:function(f){},empty:function(){}}; };
let $KEY_207 = queueFactory;
let $KEY_286 = function(x){ return 0; };
let $KEY_58 = function(a,b,c,d){ return 0; };
function $THREAD_0_33874($BS, $BT, $BU, $BV) {
			let $KEY_0 = $BV;
			let $KEY_2, $KEY_4;
			let $KEY_5 = $BS;
			let $KEY_6;
			$KEY_4 = [];
			$KEY_6 = $BT.createElement("canvas");
			$KEY_2 = $KEY_207($BW => {
				let $BX = $KEY_4;
				let $BY = $BX.push;
				let $KEY_0 = $KEY_6;
				$KEY_0.height = 96;
				$KEY_0.width = 96;
				let $BZ = [];
				try {
					let $CA = $KEY_0.getContext("2d", { willReadFrequently: true });
					$CA.font = $KEY_0.height - 2 + "px sans-serif";
					$CA.fillStyle = "#000";
					$CA.fillText($BW[0], -1, $KEY_0.height);
					let i = 0;
					for (; i < $BW[1].length; i++) {
						let $CB = $CA.getImageData($BW[1][i][0], $BW[1][i][1], 1, 1).data;
						$BZ.push($CB[0] << 24 | $CB[1] << 16 | $CB[2] << 8 | $CB[3]);
					}
					$BY.call($BX, $BZ);
					let $CC = Math.random() * 4294967296 | 0;
					let $CD = [$CC >>> 24, $CC >> 16 & 255, $CC >> 8 & 255, $CC & 255];
					$CC = $KEY_286($CC ^ 2112046103);
					let $CE = $KEY_4.length;
					let $CF = 0;
					let $CG = $KEY_58($CC, $CE, $CD, 3);
					while ($CF < $CE) {
						let $CH = $KEY_4[$CF];
						let $CI = $CH.length;
						let j = 0;
						$CG = $KEY_58($CC, $CI, $CD, $CG);
						for (; j < $CI; j++) {
							$CG = $KEY_58($CC, $CH[j], $CD, $CG);
						}
						++$CF;
					}
					$KEY_5.next([[], $CD]);
					return void 0;
				} catch (e) {
					$BY.call($BX, $BZ);
					let $CJ = Math.random() * 4294967296 | 0;
					let $CK = [$CJ >>> 24, $CJ >> 16 & 255, $CJ >> 8 & 255, $CJ & 255];
					$CJ = $KEY_286($CJ ^ 2112046103);
					let $CL = $KEY_4.length;
					let $CM = 0;
					let $CN = $KEY_58($CJ, $CL, $CK, 3);
					while ($CM < $CL) {
						let $CO = $KEY_4[$CM];
						let $CP = $CO.length;
						let k = 0;
						$CN = $KEY_58($CJ, $CP, $CK, $CN);
						for (; k < $CP; k++) {
							$CN = $KEY_58($CJ, $CO[k], $CK, $CN);
						}
						++$CM;
					}
					$KEY_5.next([[], $CK]);
					return void 0;
				}
			}, 45, $BU);
			$KEY_2.onEmpty(() => {
				$KEY_0($KEY_4);
			});
			$KEY_2.start();
			function $THREAD_6_24812() {
				if ($KEY_2) {
					$KEY_2.empty();
				}
			}
			return $THREAD_6_24812;
		}
return $THREAD_0_33874;
}
bootstrap();"##;
const FORWARD_FOR: &str = r##"function bootstrap(){
let queueFactory = function(a,b,c){ return {start:function(){},onEmpty:function(f){},empty:function(){}}; };
let $KEY_148 = queueFactory;
let $KEY_274 = function(x){ return 0; };
let $KEY_6 = function(a,b,c,d){ return 0; };
function $THREAD_5_87153($BO, $BP, $BQ, $BR) {
			let $KEY_1 = $BR;
			let $KEY_2, $KEY_3;
			let $KEY_5 = $BO;
			$KEY_2 = [];
			let $KEY_0 = $BP.createElement("canvas");
			$KEY_3 = $KEY_148($BT => {
				let $BU = $KEY_2;
				let $BV = $BU.push;
				let $KEY_3 = $KEY_0;
				$KEY_3.height = 96;
				$KEY_3.width = 96;
				let $BW = [];
				try {
					let $BX = $KEY_3.getContext("2d", { willReadFrequently: true });
					$BX.font = $KEY_3.height - 2 + "px sans-serif";
					$BX.fillStyle = "#000";
					let $BY = [$BX, $BX.fillText];
					$BY[1].call($BY[0], $BT[0], -1, $KEY_3.height);
					let i = 0;
					for (; i < $BT[1].length; i++) {
						let $BZ = $BX.getImageData($BT[1][i][0], $BT[1][i][1], 1, 1).data;
						$BW.push($BZ[0] << 24 | $BZ[1] << 16 | $BZ[2] << 8 | $BZ[3]);
					}
					$BV.call($BU, $BW);
					let $CA = Math.random() * 4294967296;
					let $CB = [($CA | 0) >>> 24, ($CA | 0) >> 16 & 255, ($CA | 0) >> 8 & 255, ($CA | 0) & 255];
					$CA = $KEY_274(($CA | 0) ^ 2491812638);
					let $CC = $KEY_2.length;
					let $CD = 0;
					let $CE = $KEY_6($CA, $CC, $CB, 3);
					while ($CD < $CC) {
						let $CF = $KEY_2[$CD];
						let $CG = $CF.length;
						let j = 0;
						$CE = $KEY_6($CA, $CG, $CB, $CE);
						for (; j < $CG; j++) {
							$CE = $KEY_6($CA, $CF[j], $CB, $CE);
						}
						++$CD;
					}
					$KEY_5.next([[], $CB]);
					return void 0;
				} catch (e) {
					$BV.call($BU, $BW);
					let $CH = Math.random() * 4294967296;
					let $CI = [($CH | 0) >>> 24, ($CH | 0) >> 16 & 255, ($CH | 0) >> 8 & 255, ($CH | 0) & 255];
					$CH = $KEY_274(($CH | 0) ^ 2491812638);
					let $CJ = $KEY_2.length;
					let $CK = 0;
					let $CL = $KEY_6($CH, $CJ, $CI, 3);
					while ($CK < $CJ) {
						let $CM = $KEY_2[$CK];
						let $CN = $CM.length;
						let k = 0;
						$CL = $KEY_6($CH, $CN, $CI, $CL);
						for (; k < $CN; k++) {
							$CL = $KEY_6($CH, $CM[k], $CI, $CL);
						}
						++$CK;
					}
					$KEY_5.next([[], $CI]);
					return void 0;
				}
			}, 45, $BQ);
			let $BS = [$KEY_3, $KEY_3.onEmpty];
			$BS[1].call($BS[0], () => {
				$KEY_1($KEY_2);
			});
			$KEY_3.start();
			function $THREAD_0_17644() {
				if ($KEY_3) {
					$KEY_3.empty();
				}
			}
			return $THREAD_0_17644;
		}
return $THREAD_5_87153;
}
bootstrap();"##;
const REVERSE_OUTER: &str = r##"function bootstrap(){
let queueFactory = function(a,b,c){ return {start:function(){},onEmpty:function(f){},empty:function(){}}; };
let $KEY_192 = queueFactory;
let $KEY_67 = function(x){ return 0; };
let $KEY_291 = function(a,b,c,d){ return 0; };
function $THREAD_5_56405($BV, $BW, $BX, $BY) {
			let $KEY_3 = $BV;
			let $KEY_4 = $BY;
			let $KEY_5;
			let $KEY_2 = [];
			let $KEY_0 = $BW.createElement("canvas");
			$KEY_5 = $KEY_192($CA => {
				let $CB = $KEY_2;
				let $CC = $KEY_2.push;
				let $KEY_1 = $KEY_0;
				$KEY_1.height = 96;
				$KEY_1.width = 96;
				let $CD = [];
				try {
					let $CE = $KEY_1.getContext("2d", { willReadFrequently: true });
					$CE.font = $KEY_1.height - 2 + "px sans-serif";
					$CE.fillStyle = "#000";
					let $CF = [$CE, $CE.fillText];
					$CF[1].call($CF[0], $CA[0], -1, $KEY_1.height);
					let i = 0;
					for (; i < $CA[1].length; i++) {
						let $CG = $CE.getImageData($CA[1][i][0], $CA[1][i][1], 1, 1).data;
						$CD.push($CG[0] << 24 | $CG[1] << 16 | $CG[2] << 8 | $CG[3]);
					}
					$CC.call($CB, $CD);
					let $CH = Math.random() * 4294967296 | 0;
					let $CI = [$CH >>> 24, $CH >> 16 & 255, $CH >> 8 & 255, $CH & 255];
					$CH = $KEY_67($CH ^ 2724439404);
					let $CJ = $KEY_2.length;
					let $CK = $KEY_291($CH, $CJ, $CI, 3);
					for (;;) {
						--$CJ;
						if (!($CJ >= 0)) break;
						let $CL = $KEY_2[$CJ];
						let $CM = $CL.length;
						let j = 0;
						$CK = $KEY_291($CH, $CM, $CI, $CK);
						for (; j < $CM; j++) {
							$CK = $KEY_291($CH, $CL[j], $CI, $CK);
						}
						$CJ = Number($CJ);
					}
					$KEY_3.next([[], $CI]);
					return void 0;
				} catch (e) {
					$CC.call($CB, $CD);
					let $CN = Math.random() * 4294967296 | 0;
					let $CO = [$CN >>> 24, $CN >> 16 & 255, $CN >> 8 & 255, $CN & 255];
					$CN = $KEY_67($CN ^ 2724439404);
					let $CP = $KEY_2.length;
					let $CQ = $KEY_291($CN, $CP, $CO, 3);
					for (;;) {
						--$CP;
						if (!($CP >= 0)) break;
						let $CR = $KEY_2[$CP];
						let $CS = $CR.length;
						let k = 0;
						$CQ = $KEY_291($CN, $CS, $CO, $CQ);
						for (; k < $CS; k++) {
							$CQ = $KEY_291($CN, $CR[k], $CO, $CQ);
						}
						$CP = Number($CP);
					}
					$KEY_3.next([[], $CO]);
					return void 0;
				}
			}, 45, $BX);
			let $BZ = [$KEY_5, $KEY_5.onEmpty];
			$BZ[1].call($BZ[0], () => {
				$KEY_4($KEY_2);
			});
			$KEY_5.start();
			function $THREAD_1_41459() {
				if ($KEY_5) {
					$KEY_5.empty();
				}
			}
			return $THREAD_1_41459;
		}
return $THREAD_5_56405;
}
bootstrap();"##;

// The collector keeps predeclared captures. Setup runs after its declaration and before it
// can be returned and invoked. Only helper bodies are replaced by inert protocol stubs; the
// collector is unchanged.
const LATE_SETUP_REVERSE: &str = r##"function bootstrap(){
let $KEY_52, $KEY_152, $KEY_190;
function $THREAD_3_77917($BS, $BT, $BU, $BV) {
			let $KEY_4 = $BS;
			let $KEY_5;
			let $KEY_6 = $BV;
			let $KEY_2 = [];
			let $KEY_1 = $BT.createElement("canvas");
			$KEY_5 = $KEY_152($BX => {
				let $BY = $KEY_2;
				let $BZ = $KEY_2.push;
				let $KEY_7 = $KEY_1;
				$KEY_7.height = 96;
				$KEY_7.width = 96;
				let $CA = [];
				try {
					let $CB = $KEY_7.getContext("2d", { willReadFrequently: true });
					$CB.font = $KEY_7.height - 2 + "px sans-serif";
					$CB.fillStyle = "#000";
					let $CC = [$CB, $CB.fillText];
					$CC[1].call($CC[0], $BX[0], -1, $KEY_7.height);
					let i = 0;
					for (; i < $BX[1].length; i++) {
						let $CD = $CB.getImageData($BX[1][i][0], $BX[1][i][1], 1, 1).data;
						$CA.push($CD[0] << 24 | $CD[1] << 16 | $CD[2] << 8 | $CD[3]);
					}
					$BZ.call($BY, $CA);
					let $CE = Math.random() * 4294967296 | 0;
					let $CF = [$CE >>> 24, $CE >> 16 & 255, $CE >> 8 & 255, $CE & 255];
					$CE = $KEY_190($CE ^ 2498694231);
					let $CG = $KEY_2.length;
					let $CH = $KEY_52($CE, $CG, $CF, 3);
					for (;;) {
						--$CG;
						if (!($CG >= 0)) break;
						let $CI = $KEY_2[$CG];
						let $CJ = $CI.length;
						$CH = $KEY_52($CE, $CJ, $CF, $CH);
						for (;;) {
							--$CJ;
							if (!($CJ >= 0)) break;
							$CH = $KEY_52($CE, $CI[$CJ], $CF, $CH);
							$CJ = Number($CJ);
						}
						$CG = Number($CG);
					}
					$KEY_4.next([[], $CF]);
					return void 0;
				} catch (e) {
					$BZ.call($BY, $CA);
					let $CK = Math.random() * 4294967296 | 0;
					let $CL = [$CK >>> 24, $CK >> 16 & 255, $CK >> 8 & 255, $CK & 255];
					$CK = $KEY_190($CK ^ 2498694231);
					let $CM = $KEY_2.length;
					let $CN = $KEY_52($CK, $CM, $CL, 3);
					for (;;) {
						--$CM;
						if (!($CM >= 0)) break;
						let $CO = $KEY_2[$CM];
						let $CP = $CO.length;
						$CN = $KEY_52($CK, $CP, $CL, $CN);
						for (;;) {
							--$CP;
							if (!($CP >= 0)) break;
							$CN = $KEY_52($CK, $CO[$CP], $CL, $CN);
							$CP = Number($CP);
						}
						$CM = Number($CM);
					}
					$KEY_4.next([[], $CL]);
					return void 0;
				}
			}, 45, $BU);
			let $BW = [$KEY_5, $KEY_5.onEmpty];
			$BW[1].call($BW[0], () => {
				$KEY_6($KEY_2);
			});
			$KEY_5.start();
			function $THREAD_3_16964() {
				if ($KEY_5) {
					$KEY_5.empty();
				}
			}
			return $THREAD_3_16964;
		}
function numericWriter(a,b,c,d){ return d; }
function reseedFactory(x){ return function(){ return 0; }; }
function queueFactory(a,b,c){ return {start:function(){},onEmpty:function(f){},empty:function(){}}; }
$KEY_52 = numericWriter;
$KEY_190 = reseedFactory;
$KEY_152 = queueFactory;
return $THREAD_3_77917;
}
bootstrap();"##;
