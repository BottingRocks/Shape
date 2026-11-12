function main(){
 let first,second,enabled,installDocument,installHost,retained,plugins,storage;
 function enable(){
  if(enabled){return;}else{
   try{function forward(event){return event;}
    retained=forward;installDocument.addEventListener("submit",forward,true);
   }catch(error){second.notify(error);return;}
   enabled=true;return;
  }
 }
 let install=enable;
 // BEFORE_BOTH
 try{storage=window.localStorage;}catch(error){
  class Base{constructor(){this.callbacks=[];}register(value){this.clear=function(){};this.callbacks.push(value);return this;}unregister(value){return this;}clear(){this.callbacks=[];return this;}notify(value){return null;}}
  installHost=window;installDocument=installHost.document;enabled=false;
  function make(BaseArg){class Derived extends BaseArg{
   constructor(){super(...arguments);}
   register(value){install();super.register(value);return this;}
  }return Derived;}
  let Constructor=make(Base);
  // CATCH_BEFORE
  first=new Constructor();second=new Constructor();
  // CATCH_AFTER
  function factory(config){
   let port=config;
   let surface,style,click,host;
   function $THREAD_4_28197(){try{
    surface.removeEventListener("click",click);
    if(!!enabled){first.clear();second.clear();
     if(retained!=null){installDocument.removeEventListener("submit",retained);}
     enabled=false;
    }
    if(surface.head){surface.head.removeChild(style);}
   }catch(error){}}
   function init(){first.register(onValue);second.register(onError);
    let useClick=true;if(useClick){surface.addEventListener("click",click);}
   }
   function onClick(event){}
   function onValue(value){}
   function onError(error){}
   click=onClick;host=port.global;surface=host.document;
   style=surface.createElement("style");
   let result={init:init};result.teardown=$THREAD_4_28197;return result;
  }
  plugins=[factory];return plugins;
 }
 class Base2{constructor(){this.callbacks=[];}register(value){this.clear=function(){};this.callbacks.push(value);return this;}unregister(value){return this;}clear(){this.callbacks=[];return this;}notify(value){return null;}}
 installHost=window;installDocument=installHost.document;enabled=false;
 function make2(BaseArg){class Derived extends BaseArg{
  constructor(){super(...arguments);}
  register(value){install();super.register(value);return this;}
 }return Derived;}
 let Constructor2=make2(Base2);
 // NORMAL_BEFORE
 first=new Constructor2();second=new Constructor2();
 // NORMAL_AFTER
 function factory2(config){
  let port=config;
  let surface,style,click,host;
  function $THREAD_4_28197(){try{
   surface.removeEventListener("click",click);
   if(!!enabled){first.clear();second.clear();
    if(retained!=null){installDocument.removeEventListener("submit",retained);}
    enabled=false;
   }
   if(surface.head){surface.head.removeChild(style);}
  }catch(error){}}
  function init(){first.register(onValue);second.register(onError);
   let useClick=true;if(useClick){surface.addEventListener("click",click);}
  }
  function onClick(event){}
  function onValue(value){}
  function onError(error){}
  click=onClick;host=port.global;surface=host.document;
  style=surface.createElement("style");
  let result={init:init};result.teardown=$THREAD_4_28197;return result;
 }
 plugins=[factory2];return plugins;
}main();
