function local0(local1) {
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
