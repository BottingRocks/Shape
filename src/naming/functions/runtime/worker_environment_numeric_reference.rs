//! Exhaustive numeric protocol grammar, authored in JavaScript for Oxc's AST.
//! These are arithmetic lowerings, not source exemplars or seed identities.
//! Each branch includes every scalar store, threshold, rounding/parity choice,
//! scratch fast path, output order and cursor operation. No candidate source is
//! used to construct this grammar. Local names describe roles for review only.

/// Sign/magnitude varints: 1 sign bit, five low payload bits, then seven-bit
/// continuation groups. The threshold widths and final cursor are explicit.
pub(super) fn integers() -> Vec<String> {
    let mut result = Vec::new();
    for sign_first in [true, false] {
        for zero_else in ["false", "value === 0"] {
            for form in 0..5 {
                let (decl, negative, positive) = if sign_first {
                    (
                        "let sign, magnitude;",
                        "sign=64; magnitude=-value;",
                        "sign=0; magnitude=value;",
                    )
                } else {
                    (
                        "let magnitude, sign;",
                        "magnitude=-value; sign=64;",
                        "magnitude=value; sign=0;",
                    )
                };
                let mut s=format!("{{let negative; if(value<0){{negative=value<0;}}else if(value===0){{negative=1/value<0;}}else{{negative={zero_else};}} {decl} if(negative){{{negative}}}else{{{positive}}}");
                for width in 1..=5 {
                    let lowering = if form == 4 {
                        if width == 5 {
                            0
                        } else {
                            1
                        }
                    } else {
                        form
                    };
                    let threshold = [32, 4096, 524288, 67108864][(width - 1).min(3)];
                    s.push_str(&if width == 1 {
                        format!("if(magnitude<{threshold}){{")
                    } else if width < 5 {
                        format!("else if(magnitude<{threshold}){{")
                    } else {
                        "else{".into()
                    });
                    let payload = if width == 1 {
                        "sign | magnitude"
                    } else {
                        "sign | (magnitude & 31 | 32)"
                    };
                    if width == 1 || lowering == 0 {
                        s.push_str(&format!(
                            "cursor=Number(position)+1; output[cursor]=({payload}) ^ mask();"
                        ));
                    } else {
                        s.push_str(&format!(
                            "let next=Number(position)+1; output[next]=({payload}) ^ mask();"
                        ));
                    }
                    for byte in 1..width {
                        let shift = [5, 12, 19, 26][byte - 1];
                        let op = if byte == 1 { ">>" } else { ">>>" };
                        let payload = if byte == width - 1 {
                            format!("magnitude {op} {shift}")
                        } else {
                            format!("magnitude {op} {shift} & 127 | 128")
                        };
                        let index = match lowering {
                            0 => "++cursor".into(),
                            1 => format!("next + {byte}"),
                            2 if width == 3 => {
                                if byte == 1 {
                                    s.push_str("cursor=next+1;");
                                    "cursor".into()
                                } else {
                                    "++cursor".into()
                                }
                            }
                            _ => {
                                if byte == width - 1 {
                                    s.push_str("cursor=next+1;");
                                    "cursor".into()
                                } else {
                                    "++next".into()
                                }
                            }
                        };
                        s.push_str(&format!("output[{index}]=({payload}) ^ mask();"));
                    }
                    if lowering == 1 && width > 1 {
                        s.push_str(&format!("cursor=next+{};", width - 1));
                    }
                    s.push('}');
                }
                s.push('}');
                result.push(s);
            }
        }
    }
    result
}

fn round_even(value: &str, floor: &str) -> String {
    format!("{value}-{floor}<0.5 ? {floor} : {value}-{floor}>0.5 ? {floor}+1 : {floor}%2 ? {floor}+1 : {floor}")
}
fn tuple_round(value: &str, rounded: &str) -> String {
    format!("if(!({value}-{rounded}<0.5)) if({value}-{rounded}>0.5){{++{rounded};}}else{{{rounded}=[{rounded},{rounded}%2]; {rounded}={rounded}[1]?{rounded}[0]+1:{rounded}[0];}}")
}
const NORMALIZE: &str = r#"
 let normalExponent=Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023);
 magnitude=magnitude/Math.pow(2,normalExponent);
 let adjusted=magnitude<1;
 magnitude=adjusted?magnitude*2:magnitude;
 adjusted=adjusted?normalExponent-1:normalExponent;
 if(magnitude>=2){++adjusted; magnitude=magnitude/2;}
"#;

/// Eleven complete arithmetic spellings. Reused bindings deliberately retain
/// their actual epochs (unit->carry, exponent->carry->exponent, tuple->scalar).
/// No arithmetic equality or IEEE replacement is assumed between spellings.
fn arithmetic(lowering: usize) -> (String, &'static str, &'static str) {
    let mut normal = String::new();
    let subnormal;
    let declarations;
    let fraction;
    let exponent;
    match lowering {
        // Scale in the magnitude binding; exponent temporarily holds below-one.
        0 => {
            declarations = "let fraction,exponent;";
            fraction = "fraction";
            exponent = "exponent";
            normal.push_str(
                r#"let normalExponent=Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023);
 magnitude=magnitude/Math.pow(2,normalExponent); exponent=magnitude<1;
 magnitude=exponent?magnitude*2:magnitude; exponent=exponent?normalExponent-1:normalExponent;
 if(magnitude>=2){++exponent;magnitude=magnitude/2;}
 let unit=Math.pow(2,52); magnitude=magnitude*unit; let rounded=Math.floor(magnitude);"#,
            );
            normal.push_str(&format!(
                "magnitude={};",
                round_even("magnitude", "rounded")
            ));
            normal.push_str("let carry=(magnitude-unit)/unit>=1;exponent=carry?exponent+1024:exponent+1023;if(exponent>2046){fraction=0;exponent=2047;}else{fraction=carry?0:magnitude-unit;}");
            subnormal = format!(
                "let rounded=Math.floor(magnitude);fraction={};exponent=0;",
                round_even("magnitude", "rounded")
            );
        }
        // Separate rounded significand / delta, then bias adjusted exponent.
        1 | 2 | 3 | 9 => {
            declarations = "let fraction,exponent;";
            fraction = "fraction";
            exponent = "exponent";
            normal.push_str(NORMALIZE);
            normal.push_str("let unit=Math.pow(2,52);");
            if lowering == 9 {
                normal.push_str("magnitude=magnitude*unit;let rounded=Math.floor(magnitude);");
                normal.push_str(&tuple_round("magnitude", "rounded"));
                normal.push_str("adjusted+=1023;exponent=(rounded-unit)/unit>=1;unit=exponent?0:rounded-unit;exponent=exponent?adjusted+1:adjusted;if(exponent>2046){fraction=0;exponent=2047;}else{fraction=unit;}");
            } else {
                normal.push_str("let rounded=Math.floor(magnitude*unit);");
                match lowering {
                    1 => {
                        normal.push_str(&format!(
                            "const roundedValue={};",
                            round_even("magnitude*unit", "rounded")
                        ));
                        normal.push_str("adjusted+=1023;let carry=(roundedValue-unit)/unit>=1;exponent=carry?adjusted+1:adjusted;if(exponent>2046){fraction=0;exponent=2047;}else{fraction=carry?0:roundedValue-unit;}");
                    }
                    2 => {
                        normal.push_str(&format!(
                            "let fractionDelta=({})-unit;",
                            round_even("magnitude*unit", "rounded")
                        ));
                        normal.push_str("adjusted+=1023;let carry=fractionDelta/unit>=1;exponent=carry?adjusted+1:adjusted;if(exponent>2046){fraction=0;exponent=2047;}else{fraction=carry?0:fractionDelta;}");
                    }
                    _ => {
                        normal.push_str(&tuple_round("magnitude*unit", "rounded"));
                        normal.push_str("rounded=rounded-unit;adjusted+=1023;unit=rounded/unit>=1;exponent=unit?adjusted+1:adjusted;if(exponent>2046){fraction=0;exponent=2047;}else{fraction=unit?0:rounded;}");
                    }
                }
            }
            subnormal = if lowering == 3 || lowering == 9 {
                format!(
                    "fraction=Math.floor(magnitude);{}exponent=0;",
                    tuple_round("magnitude", "fraction")
                )
            } else {
                format!(
                    "let rounded=Math.floor(magnitude);fraction={};exponent=0;",
                    round_even("magnitude", "rounded")
                )
            };
        }
        // Inline the ties-to-even expression twice; one spelling reuses the
        // fraction binding as the scale unit and declares exponent first.
        4 | 5 => {
            declarations = if lowering == 4 {
                "let exponent,fraction;"
            } else {
                "let fraction,exponent;"
            };
            fraction = "fraction";
            exponent = "exponent";
            normal.push_str(
                r#"let normalExponent=Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023);
 magnitude=magnitude/Math.pow(2,normalExponent);let scaled=magnitude<1;
 normalExponent=scaled?normalExponent-1:normalExponent;scaled=scaled?magnitude*2:magnitude;
 let biased=scaled>=2;scaled=biased?scaled/2:scaled;"#,
            );
            let unit = if lowering == 4 { "fraction" } else { "unit" };
            normal.push_str(&format!("{}{}=Math.pow(2,52);let rounded=Math.floor(scaled*{unit});biased=(biased?normalExponent+1:normalExponent)+1023;",if lowering==4{""}else{"let "},unit));
            let rounded = round_even(&format!("scaled*{unit}"), "rounded");
            normal.push_str(&format!("let carry=(({rounded})-{unit})/{unit}>=1;exponent=carry?biased+1:biased;if(exponent>2046){{"));
            normal.push_str(if lowering == 4 {
                "exponent=2047;fraction=0;"
            } else {
                "fraction=0;exponent=2047;"
            });
            normal.push_str(&format!("}}else{{fraction=carry?0:({rounded})-{unit};}}"));
            subnormal = if lowering == 4 {
                format!(
                    "let rounded=Math.floor(magnitude);exponent=0;fraction={};",
                    round_even("magnitude", "rounded")
                )
            } else {
                format!(
                    "let rounded=Math.floor(magnitude);fraction={};exponent=0;",
                    round_even("magnitude", "rounded")
                )
            };
        }
        // Rounded significand is retained in a separate scaled binding;
        // the original magnitude is repurposed as the fraction on both arms.
        6 => {
            declarations = "let exponent;";
            fraction = "magnitude";
            exponent = "exponent";
            normal.push_str(r#"let normalExponent=Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023);
 magnitude=magnitude/Math.pow(2,normalExponent);let scaled=magnitude<1;
 normalExponent=scaled?normalExponent-1:normalExponent;scaled=scaled?magnitude*2:magnitude;
 let biased=scaled>=2;scaled=biased?scaled/2:scaled;let unit=Math.pow(2,52);let rounded=Math.floor(scaled*unit);"#);
            normal.push_str(&format!("scaled={};", round_even("scaled*unit", "rounded")));
            normal.push_str("biased=(biased?normalExponent+1:normalExponent)+1023;let carry=(scaled-unit)/unit>=1;exponent=carry?biased+1:biased;if(exponent>2046){exponent=2047;magnitude=0;}else{magnitude=carry?0:scaled-unit;}");
            subnormal = format!(
                "let rounded=Math.floor(magnitude);exponent=0;magnitude={};",
                round_even("magnitude", "rounded")
            );
        }
        // Exponent is initialized in the normal arm itself. The above-two
        // flag becomes the normalized significand, then a separate rounded value.
        7 => {
            declarations = "let exponent;";
            fraction = "magnitude";
            exponent = "exponent";
            normal.push_str(r#"exponent=Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023);
 magnitude=magnitude/Math.pow(2,exponent);let scaled=magnitude<1;
 exponent=scaled?exponent-1:exponent;scaled=scaled?magnitude*2:magnitude;
 let normalized=scaled>=2;exponent=normalized?exponent+1:exponent;normalized=normalized?scaled/2:scaled;
 let unit=Math.pow(2,52);let rounded=Math.floor(normalized*unit);"#);
            normal.push_str(&format!(
                "rounded={};",
                round_even("normalized*unit", "rounded")
            ));
            normal.push_str("let carry=(rounded-unit)/unit>=1;exponent=carry?exponent+1024:exponent+1023;if(exponent>2046){exponent=2047;magnitude=0;}else{magnitude=carry?0:rounded-unit;}");
            subnormal = format!(
                "let rounded=Math.floor(magnitude);exponent=0;magnitude={};",
                round_even("magnitude", "rounded")
            );
        }
        // Tuple parity, with the fraction binding changing unit->carry->fraction.
        8 => {
            declarations = "let exponent,fraction;";
            fraction = "fraction";
            exponent = "exponent";
            normal.push_str(r#"exponent=Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023);
 magnitude=magnitude/Math.pow(2,exponent);let scaled=magnitude<1;
 exponent=scaled?exponent-1:exponent;scaled=scaled?magnitude*2:magnitude;
 if(scaled>=2){++exponent;scaled=scaled/2;}fraction=Math.pow(2,52);scaled=scaled*fraction;let rounded=Math.floor(scaled);"#);
            normal.push_str(&tuple_round("scaled", "rounded"));
            normal.push_str("rounded=rounded-fraction;fraction=rounded/fraction>=1;exponent=fraction?exponent+1024:exponent+1023;if(exponent>2046){exponent=2047;fraction=0;}else{fraction=fraction?0:rounded;}");
            subnormal = format!(
                "fraction=Math.floor(magnitude);{}exponent=0;",
                tuple_round("magnitude", "fraction")
            );
        }
        // Tuple rounding with fraction's carry epoch consumed before overflow.
        // The adjusted flag becomes biased exponent only after the rounding
        // tuple is reduced. Keep both stores to fraction, including the zero
        // overflow store: they are protocol operations, not an effect whitelist.
        10 => {
            declarations = "let exponent,fraction;";
            fraction = "fraction";
            exponent = "exponent";
            normal.push_str(r#"let normalExponent=Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023);
 magnitude=magnitude/Math.pow(2,normalExponent);let scaled=magnitude<1;
 normalExponent=scaled?normalExponent-1:normalExponent;scaled=scaled?magnitude*2:magnitude;
 let biased=scaled>=2;scaled=biased?scaled/2:scaled;
 let unit=Math.pow(2,52);let rounded=Math.floor(scaled*unit);"#);
            normal.push_str(&tuple_round("scaled*unit", "rounded"));
            normal.push_str("biased=(biased?normalExponent+1:normalExponent)+1023;fraction=(rounded-unit)/unit>=1;exponent=fraction?biased+1:biased;fraction=fraction?0:rounded-unit;if(exponent>2046){exponent=2047;fraction=0;}");
            subnormal = format!(
                "fraction=Math.floor(magnitude);{}exponent=0;",
                tuple_round("magnitude", "fraction")
            );
        }
        _ => unreachable!(),
    }
    (format!("{declarations} if(magnitude>=Math.pow(2,-1022)){{{normal}}}else{{magnitude=magnitude/Math.pow(2,-1074);{subnormal}}}"),fraction,exponent)
}

#[derive(Clone, Copy)]
enum Pack {
    Direct,
    Pair,
    While,
    ExitSnapshot,
}
fn packed_byte(bits: &str, index: &str) -> String {
    (0..8)
        .map(|offset| {
            let index = if offset == 0 {
                index.into()
            } else {
                format!("{index}+{offset}")
            };
            let n = format!("Number({bits}[{index}])");
            if offset == 7 {
                n
            } else {
                format!("{n} << {}", 7 - offset)
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}
fn bit_array(
    fraction: &str,
    exponent: &str,
    target: &str,
    local_output: bool,
    packing: Pack,
) -> String {
    let mut s=format!("let bits=[];let fractionCursor=51;for(;fractionCursor>=0;fractionCursor--){{bits.unshift(!!({fraction}&1));{fraction}=Math.floor({fraction}/2);}} let exponentCursor=11;");
    if matches!(packing, Pack::ExitSnapshot) {
        s.push_str(&format!("let bitReceiver,bitMethod;for(;;exponentCursor--){{bitReceiver=bits;bitMethod=bits;if(exponentCursor<=0)break;bitMethod.unshift.call(bitReceiver,!!({exponent}&1));{exponent}=Math.floor({exponent}/2);}}bitMethod.unshift.call(bitReceiver,sign);"));
    } else {
        s.push_str(&format!("for(;exponentCursor>0;exponentCursor--){{bits.unshift(!!({exponent}&1));{exponent}=Math.floor({exponent}/2);}}bits.unshift(sign);"));
    }
    s.push_str(&format!(
        "{}{target}=[];let byteCursor=56;",
        if local_output { "let " } else { "" }
    ));
    match packing {
        Pack::Direct=>s.push_str(&format!("for(;byteCursor>=0;byteCursor=byteCursor-8){{{target}.push({});}}",packed_byte("bits","byteCursor"))),
        Pack::Pair=>s.push_str(&format!("for(;byteCursor>=0;byteCursor=byteCursor-8){{let methodPair=[{target},{target}.push];methodPair[1].call(methodPair[0],{});}}",packed_byte("bits","byteCursor"))),
        Pack::While=>s.push_str(&format!("while(byteCursor>=0){{{target}.push({});byteCursor=byteCursor-8;}}",packed_byte("bits","byteCursor"))),
        Pack::ExitSnapshot=>s.push_str(&format!("let byteReceiver,lastIndex;for(;;byteCursor=lastIndex-8){{byteReceiver={target};lastIndex=byteCursor;if(!(byteCursor>=0))break;byteReceiver=[byteReceiver,byteReceiver.push];byteReceiver[1].call(byteReceiver[0],{});}}bytes=byteReceiver;",packed_byte("bits","lastIndex"))),
    }
    s
}
#[derive(Clone, Copy)]
struct FloatForm {
    arithmetic: usize,
    copy_position: bool,
    packing: Pack,
    indirect_bytes: bool,
    spare_output: bool,
    leading_output_alias: bool,
    trailing_output_alias: bool,
    const_sign: bool,
}
impl FloatForm {
    fn source(self) -> String {
        let mut s = String::from("{");
        if self.leading_output_alias {
            s.push_str("let markerOutput=output;");
        }
        if self.copy_position {
            s.push_str("cursor=position;");
        }
        s.push_str(&format!("cursor=Number(position)+1;{}[cursor]=mask()^128;let bytes{};if(floatScratch){{floatScratch[0]=value;bytes=new byteConstructor(floatScratch.buffer);}}else",if self.leading_output_alias{"markerOutput"}else{"output"},if self.spare_output{",outputAlias"}else{""}));
        if self.indirect_bytes {
            s.push_str("{let alternateBytes;if(value!==value){alternateBytes=nanData;}else{");
        } else {
            s.push_str(" if(value!==value){bytes=nanData;}else{");
        }
        s.push_str(&format!(
            "{} sign=value<0 || value===0 && 1/value<0;let magnitude=Math.abs(value);",
            if self.const_sign { "const" } else { "let" }
        ));
        let (math, fraction, exponent) = arithmetic(self.arithmetic);
        s.push_str(&math);
        let local = self.indirect_bytes || matches!(self.packing, Pack::ExitSnapshot);
        s.push_str(&bit_array(
            fraction,
            exponent,
            if local { "packed" } else { "bytes" },
            local,
            self.packing,
        ));
        if self.indirect_bytes {
            s.push_str("alternateBytes=packed;}");
            s.push_str("bytes=alternateBytes;}");
        } else {
            s.push('}');
        }
        let output = if self.spare_output {
            s.push_str("outputAlias=output;");
            "outputAlias"
        } else if self.trailing_output_alias {
            s.push_str("let finalOutput=output;");
            "finalOutput"
        } else {
            "output"
        };
        s.push_str(&format!("let outputCursor=0;for(;outputCursor<8;outputCursor++){{{output}[++cursor]=bytes[outputCursor]^mask();}}}}"));
        s
    }
}
/// Explicit supported compositions of scalar/rounding/storage lowerings.
/// Independent branch matching allows no cross-branch alias or cursor erasure.
pub(super) fn floats() -> Vec<String> {
    use Pack::*;
    let direct = FloatForm {
        arithmetic: 0,
        copy_position: true,
        packing: Direct,
        indirect_bytes: false,
        spare_output: false,
        leading_output_alias: false,
        trailing_output_alias: false,
        const_sign: false,
    };
    vec![
        FloatForm {
            packing: Pair,
            ..direct
        },
        FloatForm {
            arithmetic: 4,
            ..direct
        },
        FloatForm {
            arithmetic: 6,
            packing: Pair,
            ..direct
        },
        FloatForm {
            arithmetic: 1,
            ..direct
        },
        FloatForm {
            arithmetic: 6,
            copy_position: false,
            indirect_bytes: true,
            packing: While,
            ..direct
        },
        FloatForm {
            arithmetic: 5,
            copy_position: false,
            const_sign: true,
            leading_output_alias: true,
            trailing_output_alias: true,
            ..direct
        },
        FloatForm {
            arithmetic: 3,
            copy_position: false,
            indirect_bytes: true,
            spare_output: true,
            packing: While,
            ..direct
        },
        FloatForm {
            arithmetic: 7,
            copy_position: false,
            ..direct
        },
        FloatForm {
            arithmetic: 8,
            copy_position: false,
            packing: Pair,
            trailing_output_alias: true,
            ..direct
        },
        FloatForm {
            arithmetic: 2,
            packing: ExitSnapshot,
            ..direct
        },
        FloatForm {
            arithmetic: 9,
            copy_position: false,
            const_sign: true,
            trailing_output_alias: true,
            ..direct
        },
        FloatForm {
            arithmetic: 10,
            ..direct
        },
    ]
    .into_iter()
    .map(FloatForm::source)
    .collect()
}
