//! Finite arithmetic lowerings, authored by operation and storage role. This
//! grammar retains all operators and numeric leaves; it performs no constant
//! folding, AST rewriting, source lookup or host floating-point substitution.
//! Normalization, rounding and emission alternatives consume the entire body.
fn tie(x:&str,r:&str)->String {
    format!("{x}-{r}<.5?{r}:{x}-{r}>.5?{r}+1:{r}%2?{r}+1:{r}")
}
fn tuple(x:&str,r:&str)->String {
    format!("if(!({x}-{r}<.5)) if({x}-{r}>.5){{++{r};}}else{{{r}=[{r},{r}%2];{r}={r}[1]?{r}[0]+1:{r}[0];}}")
}
fn normal(form:usize)->String {
    let log="Math.min(Math.floor(Math.log(magnitude)/Math.LN2),1023)";
    let unit="let unit=Math.pow(2,52);";
    let (prefix,round,overflow,fraction)=match form {
        // Normalization keeps both ternary decisions in separate temporaries.
        0=>(format!("let normalExponent={log};let significand=magnitude/Math.pow(2,normalExponent);let adjusted=significand<1;normalExponent=adjusted?normalExponent-1:normalExponent;adjusted=adjusted?significand*2:significand;let upper=adjusted>=2;adjusted=upper?adjusted/2:adjusted;{unit}let rounded=Math.floor(adjusted*unit);upper=(upper?normalExponent+1:normalExponent)+1023;"),
            format!("let carry=(({})-unit)/unit>=1;exponent=carry?upper+1:upper;",tie("adjusted*unit","rounded")),
            "exponent=(1<<11)-1;fraction=0;".to_string(),format!("fraction=carry?0:({})-unit;",tie("adjusted*unit","rounded"))),
        // Exponent reused, conditional significand expression remains repeated.
        1=>(format!("exponent={log};let significand=magnitude/Math.pow(2,exponent);let adjusted=significand<1;exponent=adjusted?exponent-1:exponent;adjusted=adjusted?significand*2:significand;let upper=adjusted>=2;exponent=upper?exponent+1:exponent;{unit}let rounded=Math.floor((upper?adjusted/2:adjusted)*unit);"),
            format!("let carry=(({})-unit)/unit>=1;exponent=carry?exponent+1024:exponent+1023;",tie("(upper?adjusted/2:adjusted)*unit","rounded")),
            "fraction=0;exponent=2047;".into(),format!("fraction=carry?0:({})-unit;",tie("(upper?adjusted/2:adjusted)*unit","rounded"))),
        // Tuple rounding before subtracting the implicit leading bit.
        5=> (format!("let normalExponent={log};let significand=magnitude/Math.pow(2,normalExponent);let adjusted=significand<1;normalExponent=adjusted?normalExponent-1:normalExponent;adjusted=adjusted?significand*2:significand;let upper=adjusted>=2;{unit}let rounded=Math.floor((upper?adjusted/2:adjusted)*unit);{}upper=(upper?normalExponent+1:normalExponent)+1023;fraction=rounded-unit;unit=fraction/unit>=1;exponent=unit?upper+1:upper;",tuple("(upper?adjusted/2:adjusted)*unit","rounded")),String::new(),"exponent=(1<<11)-1;fraction=0;".into(),"fraction=unit?0:fraction;".into()),
        _=> {
            let prefix=format!("let normalExponent={log};let significand=magnitude/Math.pow(2,normalExponent);let adjusted=significand<1;significand=adjusted?significand*2:significand;adjusted=adjusted?normalExponent-1:normalExponent;if(significand>=2){{++adjusted;significand=significand/2;}}{unit}significand=significand*unit;let rounded=Math.floor(significand);");
            match form {
                2=>(prefix,format!("rounded={};adjusted+=1023;let carry=(rounded-unit)/unit>=1;exponent=carry?adjusted+1:adjusted;",tie("significand","rounded")),"fraction=0;exponent=2047;".into(),"fraction=carry?0:rounded-unit;".into()),
                3=>(prefix,format!("{}rounded=rounded-unit;adjusted+=1023;unit=rounded/unit>=1;exponent=unit?adjusted+1:adjusted;",tuple("significand","rounded")),"fraction=0;exponent=2047;".into(),"fraction=unit?0:rounded;".into()),
                _=>(prefix,format!("{}adjusted+=1023;let mantissa=rounded-unit;unit=mantissa/unit>=1;exponent=unit?adjusted+1:adjusted;",tuple("significand","rounded")),"fraction=0;exponent=2047;".into(),"fraction=unit?0:mantissa;".into()),
            }
        }
    };
    // The first lowering stores fraction before exponent on overflow.
    let overflow=if form==0 {"fraction=0;exponent=(1<<11)-1;".to_owned()}else{overflow};
    format!("{prefix}{round}if(exponent>2046){{{overflow}}}else{{{fraction}}}")
}
fn subnormal(tuple_form:bool, threshold:&str)->String {
    if tuple_form {format!("let subnormal=magnitude/Math.pow(2,{threshold});fraction=Math.floor(subnormal);{}exponent=0;",tuple("subnormal","fraction"))}
    else{format!("let subnormal=magnitude/Math.pow(2,{threshold});let rounded=Math.floor(subnormal);fraction={};exponent=0;",tie("subnormal","rounded"))}
}
fn narrow(form:usize)->(String,&'static str,&'static str,&'static str,&'static str) {
    // Return declarations, selected tag/fraction/exponent and fraction width.
    let test="(fraction&536870911)===0&&Math.abs(exponent-1023)<256";
    match form {
        0=> (format!("let selectedFraction,selectedTag,fractionWidth,exponentWidth;if({test}){{selectedTag=tags.FLOAT32;selectedFraction=fraction/536870911;if(magnitude<Math.pow(2,-126)){{magnitude=magnitude/Math.pow(2,-126-23);selectedFraction=Math.floor(magnitude);{}exponent=0;}}else{{exponent=exponent+1023-127;}}fractionWidth=23;exponentWidth=8;}}else{{selectedFraction=fraction;selectedTag=tag;fractionWidth=52;exponentWidth=11;}}",tuple("magnitude","selectedFraction")),"selectedTag","selectedFraction","exponent","fractionWidth"),
        1=> (format!("let selectedFraction,exponentWidth,selectedExponent,fractionWidth;if({test}){{tag=tags.FLOAT32;fraction=fraction/536870911;exponent=exponent+1023-127;if(magnitude<Math.pow(2,1-127)){{magnitude=magnitude/Math.pow(2,1-127-23);let rounded=Math.floor(magnitude);selectedFraction={};selectedExponent=0;}}else{{selectedFraction=fraction;selectedExponent=exponent;}}exponentWidth=8;fractionWidth=23;}}else{{selectedFraction=fraction;exponentWidth=11;selectedExponent=exponent;fractionWidth=52;}}",tie("magnitude","rounded")),"tag","selectedFraction","selectedExponent","fractionWidth"),
        2=> (format!("let selectedFraction,fractionWidth,exponentWidth;if({test}){{tag=tags.FLOAT32;fraction=fraction/536870911;exponent=exponent+1023-127;if(magnitude<Math.pow(2,-126)){{magnitude=magnitude/Math.pow(2,-126-23);let rounded=Math.floor(magnitude);selectedFraction={};exponent=0;}}else{{selectedFraction=fraction;}}fractionWidth=23;exponentWidth=8;}}else{{selectedFraction=fraction;fractionWidth=52;exponentWidth=11;}}",tie("magnitude","rounded")),"tag","selectedFraction","exponent","fractionWidth"),
        3=> (format!("let fractionWidth,selectedTag,selectedFraction,selectedExponent,exponentWidth;if({test}){{selectedTag=tags.FLOAT32;selectedFraction=fraction/536870911;selectedExponent=exponent+1023-127;if(magnitude<Math.pow(2,-126)){{magnitude=magnitude/Math.pow(2,-149);let rounded=Math.floor(magnitude);selectedFraction={};selectedExponent=0;}}fractionWidth=23;exponentWidth=8;}}else{{fractionWidth=52;selectedTag=tag;selectedFraction=fraction;selectedExponent=exponent;exponentWidth=11;}}",tie("magnitude","rounded")),"selectedTag","selectedFraction","selectedExponent","fractionWidth"),
        4=> (format!("let fractionWidth,selectedExponent,selectedTag,selectedFraction,exponentWidth;if({test}){{selectedTag=tags.FLOAT32;exponent+=1023;selectedFraction=fraction/536870911;if(magnitude<Math.pow(2,-126)){{magnitude=magnitude/Math.pow(2,-149);let rounded=Math.floor(magnitude);selectedExponent=0;selectedFraction={};}}else{{selectedExponent=exponent-127;}}fractionWidth=23;exponentWidth=8;}}else{{fractionWidth=52;selectedExponent=exponent;selectedTag=tag;selectedFraction=fraction;exponentWidth=11;}}",tie("magnitude","rounded")),"selectedTag","selectedFraction","selectedExponent","fractionWidth"),
        _=> (format!("let fractionWidth,selectedFraction,selectedTag,selectedExponent,exponentWidth;if({test}){{selectedTag=tags.FLOAT32;selectedFraction=fraction/536870911;if(magnitude<Math.pow(2,-126)){{magnitude=magnitude/Math.pow(2,1-127-23);let rounded=Math.floor(magnitude);selectedFraction={};selectedExponent=0;}}else{{selectedExponent=exponent+1023-127;}}fractionWidth=23;exponentWidth=8;}}else{{fractionWidth=52;selectedFraction=fraction;selectedTag=tag;selectedExponent=exponent;exponentWidth=11;}}",tie("magnitude","rounded")),"selectedTag","selectedFraction","selectedExponent","fractionWidth"),
    }
}
pub(super) fn protocols()->Vec<String>{
    let mut result=Vec::new();
    // Each row is a supported operation/storage lowering, not a seed key.
    for (normal_form,narrow_form,subtuple,normal_threshold,sub_threshold,bit_for,call_push) in [
        (0,0,true,"1-1023","1-1023-52",false,false),
        (1,1,false,"-1022","-1074",true,false),
        (2,2,false,"1-1023","-1074",false,false),
        (3,3,true,"-1022","-1074",false,true),
        (5,4,true,"1-1023","-1074",false,true),
        (4,5,true,"-1022","-1022-52",false,false),
    ]{
        let normal=normal(normal_form);
        let sub=subnormal(subtuple,sub_threshold);
        let (narrow,tag,fraction,exponent,width)=narrow(narrow_form);
        let bitloop=if bit_for {format!("let bitCursor={width}-1;for(;bitCursor>=0;bitCursor--){{bits.unshift({fraction}&1);{fraction}=Math.floor({fraction}/2);}}")}
            else{format!("while({width}-1>=0){{bits.unshift({fraction}&1);{fraction}=Math.floor({fraction}/2);--{width};}}")};
        let byte=(0..8).map(|i| {
            let index=if i==0{"byteCursor".into()}else{format!("byteCursor+{i}")};
            if i==7{format!("bits[{index}]")}else{format!("bits[{index}]<<{}",7-i)}
        }).collect::<Vec<_>>().join("|");
        let push=if call_push{format!("let receiver=output;output.push.call(receiver,{byte});")}else{format!("output.push({byte});")};
        let emission=if bit_for{format!("let byteCursor=0;while(byteCursor<bits.length){{{push}byteCursor+=8;}}")}
            else{format!("for(let byteCursor=0;byteCursor<bits.length;byteCursor=byteCursor+8){{{push}}}")};
        result.push(format!("let sign=value<0||value===0&&1/value<0;let magnitude=Math.abs(value);let fraction,exponent;if(magnitude===0){{fraction=0;exponent=1023;}}else if(magnitude>=Math.pow(2,{normal_threshold})){{{normal}}}else{{{sub}}}let tag=tags.DOUBLE64;{narrow}let bits=[];{bitloop}while(exponentWidth>0){{bits.unshift({exponent}&1);--exponentWidth;{exponent}=Math.floor({exponent}/2);}}bits.unshift(sign?1:0);output.push({tag});{emission}"));
    }
    // Exact source-supported folded normal boundary in the scalar rounding form.
    result.push(result[2].replace("Math.pow(2,1-1023)","Math.pow(2,-1022)"));
    // The tuple form declares exponent first, including zero/overflow stores.
    result[4]=result[4].replace("let fraction,exponent;","let exponent,fraction;")
        .replace("fraction=0;exponent=1023;","exponent=1023;fraction=0;");
    result
}
