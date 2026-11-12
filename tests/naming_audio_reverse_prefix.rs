use shape::naming::name_threads;

const LOCAL: &str = include_str!("fixtures/program/audio-reverse-prefix/fractional-hash-local.js");
const RESEED: &str = include_str!("fixtures/program/audio-reverse-prefix/fractional-reseed.js");

fn normalized(reseed: bool) -> String {
    let (body, replacements): (&str, &[(&str, &str)]) = if reseed {
        (
            RESEED,
            &[
                ("$THREAD_0_19371", "$THREAD_0_771"),
                ("$DI", "input"),
                ("$KEY_5", "samples"),
                ("$DJ", "word"),
                ("$DK", "output"),
                ("$DL", "bytes"),
                ("$DM", "cursor"),
                ("$DN", "length"),
                ("$KEY_132", "hashFactory"),
                ("$KEY_337", "mix"),
                ("$KEY_30", "prefix"),
                ("$KEY_27", "observer"),
            ],
        )
    } else {
        (
            LOCAL,
            &[
                ("$THREAD_9_62625", "$THREAD_0_771"),
                ("$CT", "input"),
                ("$KEY_4", "samples"),
                ("$CU", "word"),
                ("$CV", "output"),
                ("$CW", "bytes"),
                ("$KEY_2", "hash"),
                ("$CX", "cursor"),
                ("$CY", "length"),
                ("$KEY_390", "hashFactory"),
                ("$KEY_243", "mix"),
                ("$KEY_8", "prefix"),
                ("$KEY_15", "observer"),
            ],
        )
    };
    // Longer generated identifiers must be replaced before their prefixes.
    let mut replacements = replacements.to_vec();
    replacements.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
    replacements
        .iter()
        .fold(body.to_owned(), |body, (from, to)| body.replace(from, to))
}

fn carrier(body: &str) -> String {
    format!(
        r#"function main() {{
        let prefix = [0], observer = {{next() {{}}, complete() {{}}}};
        function hashFactory(value) {{return value;}}
        function mix(hash, value, bytes, cursor) {{return cursor;}}
        {body}
        return $THREAD_0_771;
    }} main();"#
    )
}

fn check_source(source: &str, expected: bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        parsed.errors.is_empty(),
        "invalid fixture: {:?}",
        parsed.errors
    );
    let result = name_threads(source, true).unwrap();
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_0_771")
        .unwrap();
    assert_eq!(
        outcome.applied && outcome.proposed_name.as_deref() == Some("emitSeededHashEncoding"),
        expected,
        "{source}\n{outcome:?}"
    );
}

#[test]
fn audio_prefix_positive_variants() {
    for reseed in [false, true] {
        let body = normalized(reseed);
        check_source(&carrier(&body), true);
        // Literal seed salts and scoped participant spellings are not gates.
        check_source(
            &carrier(
                &body
                    .replace("word", "nonceWord")
                    .replace("samples", "channelSamples")
                    .replace("2099515294", "123")
                    .replace("1694101466", "456"),
            ),
            true,
        );
        check_source(
            &carrier(&body.replace(
                "Math.random() * 4294967296;",
                "Math.random() * 4294967296 | 0;",
            )),
            true,
        );
    }
}

#[test]
fn audio_prefix_negative_relationships() {
    for reseed in [false, true] {
        let body = normalized(reseed);
        for (from, to) in [
            ("getChannelData(0)", "getChannelData(1)"),
            ("input.getChannelData(0)", "prefix.getChannelData(0)"),
            ("Array.from(samples)", "Array.from([])"),
            ("samples.length", "bytes.length"),
            ("prefix[0]", "input[0]"),
            ("bytes, 3)", "bytes, 4)"),
            ("length - 1 >= 0", "length >= 0"),
            ("samples[length - 1]", "samples[length]"),
            ("--length", "++length"),
            ("bytes, cursor)", "bytes, 3)"),
            ("observer.complete()", "prefix.complete()"),
            (
                "observer.next([output, bytes])",
                "observer.next([output, samples])",
            ),
            (
                "observer.next([output, bytes])",
                "observer.next([samples, bytes])",
            ),
            ("let output = [];", "let output = [1];"),
            (
                "while (length - 1 >= 0)",
                "if (false) while (length - 1 >= 0)",
            ),
            ("--length;", "--length; prefix.push(1);"),
            ("--length;", "--length; word = 0;"),
            ("--length;", "--length; let samples = [];"),
        ] {
            assert!(body.contains(from), "missing mutation {from}");
            check_source(&carrier(&body.replace(from, to)), false);
        }
    }
}

#[test]
fn audio_prefix_negative_nested_donors_and_mutable_helpers() {
    for reseed in [false, true] {
        let body = normalized(reseed);
        // The whole required loop belongs to another callable, never the owner.
        let start = body.find("while (").unwrap();
        let loop_end = body[start..].find('}').unwrap() + start + 1;
        let nested = format!(
            "{}function donor() {{ {} }}{}",
            &body[..start],
            &body[start..loop_end],
            &body[loop_end..]
        );
        check_source(&carrier(&nested), false);
        let sibling = format!(
            "function $THREAD_0_771(input) {{ return donor(input); }}\n{}",
            body.replace("$THREAD_0_771", "donor")
        );
        check_source(&carrier(&sibling), false);
        let source = carrier(&body);
        check_source(
            &source.replace(
                "return $THREAD_0_771;",
                "mix = other; return $THREAD_0_771;",
            ),
            false,
        );
        check_source(
            &source.replace(
                "return $THREAD_0_771;",
                "hashFactory = other; return $THREAD_0_771;",
            ),
            false,
        );
        // Unresolved host names in the owned body must remain unshadowed.
        check_source(
            &source.replace("let prefix = [0]", "let Math = other, prefix = [0]"),
            false,
        );
        check_source(
            &source.replace("let prefix = [0]", "let Array = other, prefix = [0]"),
            false,
        );
    }
}
