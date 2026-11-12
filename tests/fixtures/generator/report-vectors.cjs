// Synthetic report answers from the unchanged pasted factory and shipped source dependencies.
'use strict';
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const sha=x=>crypto.createHash('sha256').update(x).digest('hex');
const files=['payload-factory.js','payload-cipher.js','superpack-reference.cjs'];
const provenance=JSON.parse(fs.readFileSync(path.join(__dirname,'report-provenance.json')));
for(const file of files)assert.equal(sha(fs.readFileSync(path.join(__dirname,file))),provenance.files[file]);
const profile={format:'shape-byte-codec/1',cipher:{ivs:[522575410,361402133,1980070771,3031302865],counter:307069178630199,double_rounds:5},alphabet:'0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz=_',keys:[0,1,2,3,4,5,6,7]};
const ctx=vm.createContext({module:{exports:{}}});
for(const file of ['superpack-reference.cjs','payload-cipher.js'])vm.runInContext(fs.readFileSync(path.join(__dirname,file),'utf8'),ctx,{filename:file});
vm.runInContext(`
const api=module.exports, cipher=createChaChaLikeStreamCipher();
const RealDate=Date; globalThis.Date=class extends RealDate {constructor(...a){super(...(a.length?a:[1700000000123]));}};
Math.random=()=>0.25;globalThis.performance={now:()=>12345};globalThis.crypto={getRandomValues:a=>{a[0]=0x89abcdef;return a;}};
globalThis.$KEY_293=41;globalThis.$KEY_93=api.signalOptions().extensions[0];globalThis.$KEY_395=api.signalOptions().extensions[3735928559];globalThis.$KEY_300=api.missingSentinel;
globalThis.$KEY_397=(v,o)=>api.packValue(v,o);
globalThis.$KEY_77=(bytes,keys,a,b)=>{globalThis.plaintext=Array.from(bytes);globalThis.seeds=[a,b];return cipher(bytes,keys,a,b);};
function js(v){switch(v.kind){case 'undefined':return undefined;case 'null':return null;case 'bool':case 'string':return v.value;case 'number':{const a=new Uint8Array(v.value.match(/../g).map(x=>parseInt(x,16)));return new DataView(a.buffer).getFloat64(0);}case 'array':return v.value.map(js);case 'object':{const o=Object.create(null);for(const [k,x]of v.value)Object.defineProperty(o,k,{value:js(x),enumerable:true});return o;}case 'binary':return Uint8Array.from(v.value).buffer;case 'date':return new Date(v.value);case 'missing_sentinel':return api.missingSentinel;default:throw Error(v.kind);}}
`,ctx);
vm.runInContext(fs.readFileSync(path.join(__dirname,'payload-factory.js'),'utf8'),ctx,{filename:'payload-factory.js'});
const num=x=>{const b=Buffer.alloc(8);b.writeDoubleBE(x);return {kind:'number',value:b.toString('hex')}};
const arr=value=>({kind:'array',value}),str=value=>({kind:'string',value}),obj=value=>({kind:'object',value});
const empty=()=>({errors:[],positionalSignals:[],signals:[],keyedSignals:[],seeds:[1200151530,1200151531],keysets:[]});
const cases=[];const add=(name,fields)=>cases.push({name,report:{...empty(),...fields}});
add('empty',{});add('sentinels',{positionalSignals:[null],keyedSignals:[[7,null],[7,null],[0,null],[4294967295,null]]});
const special=[{kind:'undefined'},{kind:'null'},... [0,-0,NaN,Infinity,-Infinity].map(num),{kind:'bool',value:true},str('quoted [{"\\😀\0'),{kind:'binary',value:[0,128,255]},{kind:'date',value:123456789},obj([['b',num(2)],['a',str('hello')]])];
add('four-fields-specials-raw',{errors:special,positionalSignals:[null,[special,[0,128,255,17]], [[],[3]]],signals:special,keyedSignals:[[128,null],[7,[special,[255,0]]],[7,[[],[128]]],[0,null]]});
add('ordered-omitted-keysets',{signals:[obj([['b',num(2)],['a',{kind:'missing_sentinel'}]]),obj([['z',str('new')]])],keysets:[['a','b'],['unused']]});
for(const depth of [98,99,100,101,103])for(const objects of [false,true]){
 let v=arr(special);for(let i=0;i<depth;i++)v=objects?obj([['x',v]]):arr([v]);
 add(`depth-${depth}-${objects?'objects':'arrays'}`,{signals:[v]});
}
let deep=num(5);for(let i=0;i<103;i++)deep=arr([deep]);
add('defaults-do-not-truncate',{errors:[deep],positionalSignals:[[[deep],[]]],keyedSignals:[[7,[[deep],[]]]]});
for(const n of [127,128,16383,16384]){
 const values=[str('x'.repeat(n-3))], extra=Array.from({length:n},(_,i)=>(i*7)&255);
 for(const channel of ['errors','signals','positional','keyed','positional-extra','keyed-extra']){
  const fields=channel==='errors'?{errors:values}:channel==='signals'?{signals:values}:channel==='positional'?{positionalSignals:[[values,[]]]}:channel==='keyed'?{keyedSignals:[[0,[values,[]]]]}:channel==='positional-extra'?{positionalSignals:[[[],extra]]}:{keyedSignals:[[0,[[],extra]]]};
  add(`${channel}-length-${n}`,fields);
 }
}
for(const n of [0,1,127,128,16383,16384,2097151,2097152,268435455,268435456,4294967295])add('name-'+n,{keyedSignals:[[n,null]]});
for(const n of [1,2,3])add('base64-tail-'+n,{positionalSignals:[[[],Array(n).fill(255)]]});
const answers=cases.map(({name,report})=>{
 ctx.spec=report;ctx.profile=profile;
 vm.runInContext(`$KEY_293=41;var entry=e=>e===null?$KEY_300:[e[0].map(js),e[1]];globalThis.report={errors:spec.errors.map(js),positionalSignals:spec.positionalSignals.map(entry),signals:spec.signals.map(js),keyedSignals:spec.keyedSignals.map(([n,e])=>[n,entry(e)])};globalThis.output=createSignalPayloadEncoder(profile.alphabet,spec.keysets,null,profile.keys)(report);`,ctx);
 assert.deepEqual(Array.from(ctx.seeds),report.seeds);
 const text=ctx.output;
 return {name,report,payload_sha256:sha(text),plaintext_sha256:sha(Buffer.from(ctx.plaintext)),...(text.length<2048?{base64:text}:{})};
});
const text=JSON.stringify({profile,answers})+'\n';
const output=path.join(__dirname,'report-known-answers.json');
if(process.argv.includes('--check'))assert.equal(fs.readFileSync(output,'utf8'),text);
else fs.writeFileSync(output,text);
console.log(`Verified ${answers.length} synthetic report answers against unchanged source factory with pinned entropy`);
