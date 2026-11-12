function main(){
let config={},cache={},registry={};
let state={},latest={},signalNames={};
let errors={toArray:function(){return [];}};
function $THREAD_8_505(options){
let opts=options===void 0?{}:options;
let filter=opts.signalFilter===void 0?config:opts.signalFilter;
if(opts.notifyLastChance===void 0||opts.notifyLastChance){
filter.forEach(key=>{
if(state[key]!=null){
state[key].call(void 0);
}
});
}
let mapped;
if(opts.includeDebugNames!==void 0&&opts.includeDebugNames){
mapped=filter.map(key=>{return {value:latest[key],signal:signalNames[key]};});
}else{
mapped=filter.map(key=>latest[key]);
}
let plain=[];
let positional=[];
let keyed=[];
let i=0;
for(;i<mapped.length;i++){
let record=registry[filter[i]];
let sink,push,item;
if(Object.prototype.hasOwnProperty.call(record,"name")){
sink=keyed;
push=sink.push;
item=[record.name,mapped[i]];
}else{
push=positional.push;
sink=positional;
item=mapped[i];
}
push.call(sink,item);
}
return {signals:plain,positionalSignals:positional,keyedSignals:keyed,errors:errors.toArray()};
}
return $THREAD_8_505;
}
main();