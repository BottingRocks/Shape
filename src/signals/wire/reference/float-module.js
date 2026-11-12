function main() {
__HASH__
__STREAM__
const local4 = function unit(s) { return s.charCodeAt(); };
const local5 = true;
let tokenState;
function cipher(keys,a,b) {
  let state=[1,2,3,4,keys[0],keys[1],keys[2],keys[3],keys[4],keys[5],keys[6],keys[7],0,0,a,b];
  let counter=5; state[12]=counter|0;
  while(a){a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;}
}
function wrap(t) { return function(o) { t(o); }; }
function Session(token) {
  let context={global:globalThis,transactionToken:void 0};
  context.transactionToken=token;
  tokenState=tokenHash(context.transactionToken);
__NUMBER__
let local10,local11,local12;
const U8=Uint8Array; local11=typeof U8==="function"?U8:null;
const F64=Float64Array; local10=local11&&typeof F64==="function"?new F64(1):null;
local12=local10?null:[];
  const document={reading:1.5};
  let task=wrap(function(o) {
    let seed=Math.random()*4294967296|0;
    let mask=streamFactory(seed^17);
    let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
    let p=3; p=numberWriter(mask,document.reading,b,p);
    o.next([[],b]);
  });
  let literal=wrap(function(o) {
    let seed=Math.random()*4294967296|0;
    let mask=streamFactory(seed^17);
    let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
    b[4]=9^mask();
    o.next([[],b]);
  });
  let registry=[{ctor:task,priority:false},{ctor:literal,priority:false}];
  registry.forEach(e=>{let t=e.ctor;if(t)t({next:function(value){}});});
  let filter=[0,1];
  function route(options) {let f=options.signalFilter?options.signalFilter:filter;return registry[f[0]];}
  let indices=[],table=[];indices.map(i=>table[i]);
  context.c="";context.d="";context.z="";
  send("x-example-"+"a","payload",1000);
  return {init:function(){},teardown:function(){}};
}
return {init:function(token){return new Session(token);}};
}
main().init("ab");
