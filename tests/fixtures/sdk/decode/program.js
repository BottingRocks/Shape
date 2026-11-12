function main() {
function integerWriter(local1, local2, local3, local4) {
    let local5;
    if ((local2 | 0) === local2) {
      let local6 = local2 < 0;
      const local7 = local6 ? local3 : (local6 = local2 === 0 && 1 / local2 < 0, local3);
      const local8 = local6 ? -local2 : local2;
      if (local8 < 32) {
        local5 = Number(local4) + 1;
        local7[local5] = ((local6 ? 64 : 0) | local8) ^ local1();
      } else if (local8 < 4096) {
        local5 = Number(local4) + 1;
        local7[local5] = ((local6 ? 64 : 0) | (local8 & 31 | 32)) ^ local1();
        local7[++local5] = local8 >> 5 ^ local1();
      } else if (local8 < 524288) {
        local5 = Number(local4) + 1;
        local7[local5] = ((local6 ? 64 : 0) | (local8 & 31 | 32)) ^ local1();
        local7[++local5] = (local8 >> 5 & 127 | 128) ^ local1();
        local7[++local5] = local8 >>> 12 ^ local1();
      } else if (local8 < 67108864) {
        local5 = Number(local4) + 1;
        local7[local5] = ((local6 ? 64 : 0) | (local8 & 31 | 32)) ^ local1();
        local7[++local5] = (local8 >> 5 & 127 | 128) ^ local1();
        local7[++local5] = (local8 >>> 12 & 127 | 128) ^ local1();
        local7[++local5] = local8 >>> 19 ^ local1();
      } else {
        local5 = Number(local4) + 1;
        local7[local5] = ((local6 ? 64 : 0) | (local8 & 31 | 32)) ^ local1();
        local7[++local5] = (local8 >> 5 & 127 | 128) ^ local1();
        local7[++local5] = (local8 >>> 12 & 127 | 128) ^ local1();
        local7[++local5] = (local8 >>> 19 & 127 | 128) ^ local1();
        local7[++local5] = local8 >>> 26 ^ local1();
      }
    } else {
      local5 = Number(local4) + 1;
      local3[local5] = local1() ^ 128;
      let local9;
      if (local10) {
        local10[0] = local2;
        local9 = new local11(local10.buffer);
      } else if (local2 !== local2) {
        local9 = local12;
      } else {
        let i = local2 < 0 || local2 === 0 && 1 / local2 < 0;
        let local13 = Math.abs(local2);
        let local14;
        if (local13 >= Math.pow(2, -1022)) {
          let local15 = Math.min(Math.floor(Math.log(local13) / Math.LN2), 1023);
          let local16 = local13 / Math.pow(2, local15);
          let local17 = local16 < 1 ? (local16 = local16 * 2, local15 - 1) : local15;
          if (local16 >= 2) {
            ++local17;
            local16 = local16 / 2;
          }
          local14 = Math.pow(2, 52);
          local16 = local16 * local14;
          let local18 = Math.floor(local16);
          local16 = local16 - local18 < .5 ? local18 : local16 - local18 > .5 ? local18 + 1 : local18 % 2 ? local18 + 1 : local18;
          local16 = (local16 - local14) / local14 >= 1 ? (local17 = local17 + 1024, 0) : (local17 = local17 + 1023, local16 - local14);
          local14 = local17 > 2046 ? (local13 = 2047, 0) : (local13 = local17, local16);
        } else {
          local14 = local13 / Math.pow(2, -1074);
          let local19 = Math.floor(local14);
          local14 = local14 - local19 < .5 ? local19 : local14 - local19 > .5 ? local19 + 1 : local19 % 2 ? local19 + 1 : local19;
          local13 = 0;
        }
        let local20 = 51;
        let local21 = [];
        while (local20 >= 0) {
          local21.unshift(!!(local14 & 1));
          local14 = Math.floor(local14 / 2);
          --local20;
        }
        local20 = 11;
        while (local20 > 0) {
          local21.unshift(!!(local13 & 1));
          --local20;
          local13 = Math.floor(local13 / 2);
        }
        local21.unshift(i);
        i = 56;
        local13 = [];
        for (; i >= 0; i = i - 8) {
          local13.push(Number(local21[i]) << 7 | Number(local21[i + 1]) << 6 | Number(local21[i + 2]) << 5 | Number(local21[i + 3]) << 4 | Number(local21[i + 4]) << 3 | Number(local21[i + 5]) << 2 | Number(local21[i + 6]) << 1 | Number(local21[i + 7]));
        }
        local9 = local13;
      }
      let j = 0;
      for (; j < 8; j++) {
        local3[++local5] = local9[j] ^ local1();
      }
    }
    return local5;
  }

function tokenHash(local1) {
      if (typeof local1 === "string") {
        let local2 = local1.length >> 1 << 1;
        let i = 0;
        let local3 = 0;
        for (; i < local2; i = i + 2) {
          local3 = (local3 << 5) - local3 + (local4(local1[i]) << 16 | local4(local1[i + 1])) | 0;
        }
        if (local2 !== local1.length) {
          return (local3 << 5) - local3 + (local4(local1[local2]) << 16) | 0;
        } else {
          return local3;
        }
      }
      if (local5 && ArrayBuffer.isView(local1)) {
        let local6 = local1.byteLength % 4;
        let local7;
        if (local6 === 0) {
          local7 = local1.buffer;
        } else {
          local7 = new ArrayBuffer(local1.byteLength + (4 - local6));
          new Int8Array(local7).set(new Int8Array(local1.buffer));
        }
        local6 = new Int32Array(local7);
        local7 = local6.length;
        let local8 = 0;
        for (let j = 0; j < local7; j++) {
          local8 = (local8 << 5) - local8 + local6[j] | 0;
        }
        return local8;
      } else {
        throw new TypeError();
      }
    }

function streamFactory(local1) {
      let local2, local3, local4, local5;
      function local6() {
        local4 = (local4 + 1) % 4;
        if (local4 === 0) {
          local3 = local5 + local2;
          let local7 = local2;
          local2 = local7 ^ local5;
          let local8 = local5;
          local5 = local7 ^ local5 ^ (local8 << 26 | local8 >>> 6) ^ local2 << 9;
          local8 = local2;
          local2 = local8 << 13 | local8 >>> 19;
          return local3 & 255;
        }
        if (local4 === 1) {
          return local3 >> 8 & 255;
        }
        if (local4 === 2) {
          return local3 >> 16 & 255;
        }
        if (local4 === 3) {
          return local3 >>> 24 & 255;
        } else {
          return void 0;
        }
      }
      local5 = tokenState;
      local2 = local1;
      local4 = -1;
      local6();
      local6();
      local6();
      local6();
      return local6;
    }

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
let sharedNumber; sharedNumber=integerWriter; const navigator={deviceMemory:9};
  let context={global:globalThis,transactionToken:void 0};
  context.transactionToken=token;
  tokenState=tokenHash(context.transactionToken);
  const document={hasStorageAccess:true};
  let task=wrap(function(o) {
    let seed=Math.random()*4294967296|0;
    let mask=streamFactory(seed^17);
    let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
    b[4]=(("hasStorageAccess" in document)|("requestStorageAccess" in document)<<1)^mask();
    o.next([[],b]);
  });
  let literal=wrap(function(o) {
    let seed=Math.random()*4294967296|0;
    let mask=streamFactory(seed^17);
    let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
    let callable; callable=sharedNumber; callable=callable(mask,navigator.deviceMemory,b,3); callable=0;
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
