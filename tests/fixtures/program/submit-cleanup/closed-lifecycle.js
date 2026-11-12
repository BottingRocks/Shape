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
  function Base(){this.callbacks=[];}
  function register(value){this.callbacks.push(value);return this;}
  function unregister(value){return this;}
  function clear(){this.callbacks=[];return this;}
  function notify(value){return null;}
  Base.prototype.register=register;Base.prototype.unregister=unregister;
  Base.prototype.clear=clear;Base.prototype.notify=notify;
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
 function Base2(){this.callbacks=[];}
 function register2(value){this.callbacks.push(value);return this;}
 function unregister2(value){return this;}
 function clear2(){this.callbacks=[];return this;}
 function notify2(value){return null;}
 Base2.prototype.register=register2;Base2.prototype.unregister=unregister2;
 Base2.prototype.clear=clear2;Base2.prototype.notify=notify2;
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
