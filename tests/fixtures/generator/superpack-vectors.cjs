// Generate fixed answers using ORIGINAL source function bodies, not a JS rewrite.
const fs=require('node:fs');
const path=require('node:path');
const api=require('./superpack-reference.cjs');
const alternateApi=require('./superpack-reference-alternate.cjs');
const num=v=>{const b=Buffer.alloc(8);b.writeDoubleBE(v);return {kind:'number',value:b.toString('hex')};};
const str=v=>({kind:'string',value:v});
const bool=v=>({kind:'bool',value:v});
const arr=v=>({kind:'array',value:v});
const obj=v=>({kind:'object',value:v});
function js(v, runtime=api){switch(v.kind){
 case 'undefined':return undefined;case 'null':return null;case 'bool':case 'string':return v.value;
 case 'number':return Buffer.from(v.value,'hex').readDoubleBE();
 case 'array':return v.value.map(x=>js(x,runtime));
 case 'object':{const o=Object.create(null);for(const [k,x] of v.value) Object.defineProperty(o,k,{value:js(x,runtime),enumerable:true});return o;}
 case 'binary':return Uint8Array.from(v.value).buffer;case 'date':return new Date(v.value);
 case 'missing_sentinel':return runtime.missingSentinel;
}}
const vectors=[];
function add(name,value,options={},mode='normal'){
 const opts=options.depth_bound!=null?api.signalOptions(options.keysets_to_omit||[],options.depth_bound):{keysetsToOmit:options.keysets_to_omit};
 let bytes=api.packValue(js(value),opts);
 const alternateOpts=options.depth_bound!=null?alternateApi.signalOptions(options.keysets_to_omit||[],options.depth_bound):{keysetsToOmit:options.keysets_to_omit};
 const alternateBytes=alternateApi.packValue(js(value,alternateApi),alternateOpts);
 if(Buffer.from(bytes).compare(Buffer.from(alternateBytes)))throw Error('Alternate encoder mismatch: '+name);
 vectors.push({name,request:{value,options},hex:Buffer.from(bytes).toString('hex'),decodable:mode!=='truncated_metadata'});
}
for(const v of [0,1,63,64,16383,16384,65535,65536,16777215,16777216,4294967295,4294967296,
 2**53,2**64-4096,2**64-2048,2**64,-1,-15,-16,-255,-256,-65535,-65536,-4294967295,-4294967296,
 -(2**64-4096),-0,NaN,Infinity,-Infinity,1.5,0.1,Math.fround(0.1),Number.MIN_VALUE,2**-149,Number.MAX_VALUE]) add('number:'+String(v)+(Object.is(v,-0)?'-zero':''),num(v));
for(const v of [{kind:'undefined'},{kind:'null'},bool(true),bool(false),{kind:'missing_sentinel'}])
 add(v.kind+(v.value===undefined?'':v.value),v,v.kind==='missing_sentinel'?{depth_bound:100,missing_sentinel_extension:true}:{});
for(const s of ['', 'hé😀', 'a\0b','x'.repeat(31),'x'.repeat(32),'\0'+'x'.repeat(31),'\0'+'x'.repeat(255),'x'.repeat(65536),'\0'+'x'.repeat(65535)]) add('string:'+s.length,str(s));
for(const count of [0,1,15,16,31,32,255,256]) {
 add('array:'+count,arr(Array.from({length:count},(_,i)=>num(i%3))));
 if(count) add('bool_array:'+count,arr(Array.from({length:count},(_,i)=>bool(i%3===0))));
}
for(const v of [obj([]),obj([['b',num(2)],['a',num(1)]]),obj([['b',bool(false)],['a',bool(true)]]),
 obj([['2',str('two')],['10',str('ten')],['__proto__',str('p')],['constructor',num(4)],['😀',num(5)],['\ue000',num(6)]]),
 arr([obj([['a',num(1)],['b',num(2)]]),obj([['b',num(3)],['a',num(4)]])]),
 arr(['hello','hello','__proto__','__proto__','toString','toString','1234','1234'].map(str)),
 arr(['9000','1000','9000','1000','xxxx','xxxx'].map(str)),
 arr(Array.from({length:260},(_,i)=>arr([str('s'+i.toString().padStart(4,'0')),str('s'+i.toString().padStart(4,'0'))]))),
 obj([['hello',str('hello')],['other',arr([str('hello'),str('hello')])]])]) add('map_lut:'+vectors.length,v);
add('lut_UTF16_not_UTF8',arr(['😀😀','😀😀','éé','éé'].map(str)));
add('lut_negative_savings_still_selected',arr(Array(8).fill(str('a'))));
add('boolean_map_many',obj(Array.from({length:257},(_,i)=>['k'+i,bool(i%3===0)])));
add('preloaded',obj([['a',num(1)],['b',num(2)]]),{keysets_to_omit:[['a','b']]});
add('preloaded_plus_new',arr([obj([['a',num(1)]]),obj([['z',num(2)]])]),{keysets_to_omit:[['a']]});
for(const depth of [0,1,2,3,100]){
 add('depth-array:'+depth,arr([arr([num(2)])]),{depth_bound:depth,missing_sentinel_extension:true});
 add('depth-map:'+depth,obj([['a',obj([['b',num(1)]])]]),{depth_bound:depth,missing_sentinel_extension:true},depth===1?'truncated_metadata':'normal');
}
add('binary',{kind:'binary',value:[0,63,64,127,128,255]});
for(const ms of [0,1,4294967295,4294967296,2**48-1]) add('date:'+ms,{kind:'date',value:ms});
let text=JSON.stringify(vectors,null,2);
const outputPath=path.join(__dirname,'superpack-known-answers.json');
if(process.argv.includes('--check')) {
 if(fs.readFileSync(outputPath,'utf8')!==text+'\n')throw Error('Fixed answers differ from original source encoders');
 console.log(`Verified ${vectors.length} vectors against both original source encoders`);
} else {
 fs.writeFileSync(outputPath,text+'\n');
 console.log(`Wrote ${vectors.length} source vectors`);
}
