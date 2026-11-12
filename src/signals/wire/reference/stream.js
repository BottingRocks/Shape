function local0(local1) {
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
      local5 = local9;
      local2 = local1;
      local4 = -1;
      local6();
      local6();
      local6();
      local6();
      return local6;
    }
