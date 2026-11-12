function createNativeNumberWriter() {
  const local10 = new globalThis.Float64Array(1);
  const local11 = globalThis.Uint8Array;
  const local12 = [];
function local0(local1, local2, local3, local4) {
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

  return local0;
}
