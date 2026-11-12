function local0(local1) {
    let local2 = local1;
    let local3 = Object.create(null);
    let local4 = 0;
    function local5(local6, local7, local8) {
      let local9;
      if (local3[local6] != null) {
        local9 = Number(local8) + 1;
        local7[local9] = local2() ^ 129;
        local9 = local10(local2, local3[local6], local7, local9);
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
              local14 = (local14 & 63 | 128) ^ local2();
              local7[local13] = local14;
            }
          }
          ++local12;
        }
        local9 = Number(local13) + 1;
        local7[local9] = local2();
        local13 = local3;
        local12 = Number(local4);
        local4 = local12 + 1;
        local13[local6] = local12;
      }
      return local9;
    }
    return local5;
  }
