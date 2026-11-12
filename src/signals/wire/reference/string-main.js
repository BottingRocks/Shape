function local0(local1) {
      let local2 = local1;
      let local3, local4;
      local4 = Object.create(null);
      local3 = 0;
      function local5(local6, local7, local8) {
        if (local4[local6] != null) {
          let local9 = Number(local8) + 1;
          local7[local9] = local2() ^ 129;
          return local10(local2, local4[local6], local7, local9);
        } else {
          let local11 = local6.length;
          let local12 = 0;
          let local13 = local8;
          while (local12 < local11) {
            let local14 = local15(local6[local12], 0);
            if (local14 === 0) {
              local7[++local13] = local2() ^ 192;
              local7[++local13] = local2() ^ 128;
            } else if (local14 < 128) {
              local7[++local13] = local14 ^ local2();
            } else if (local14 < 2048) {
              local7[++local13] = (local14 >> 6 | 192) ^ local2();
              local7[++local13] = (local14 & 63 | 128) ^ local2();
            } else {
              const local16 = local14 >= 55296 && local14 <= 57343 && local14 < 56320 && local12 + 1 < local11 ? local15(local6[local12 + 1], 0) : null;
              if (local16 == null || local16 < 56320 || local16 > 57343) {
                local7[++local13] = (local14 >> 12 | 224) ^ local2();
                local7[++local13] = (local14 >> 6 & 63 | 128) ^ local2();
                local7[++local13] = (local14 & 63 | 128) ^ local2();
              } else {
                ++local12;
                local14 = 65536 + (local14 - 55296 << 10) + (local16 - 56320);
                local7[++local13] = (local14 >> 18 | 240) ^ local2();
                local7[++local13] = (local14 >> 12 & 63 | 128) ^ local2();
                local7[++local13] = (local14 >> 6 & 63 | 128) ^ local2();
                ++local13;
                local14 = local14 & 63 | 128;
                local7[local13] = local14 ^ local2();
              }
            }
            ++local12;
          }
          local7[++local13] = local2();
          local12 = local4;
          local11 = Number(local3);
          local3 = local11 + 1;
          local12[local6] = local11;
          return local13;
        }
      }
      return local5;
    }
