//! Complete numeric protocol alternatives. All operations, scopes, captures,
//! returns and cursor effects are retained by the typed AST projector.
//! Role identifiers are grammar metavariables; no seed identity is consulted.

pub(super) const FORMS: &[&str] = &[
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        const local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local6 = local10 ? local6 * 2 : local6;
          local10 = local10 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local10;
            local6 = local6 / 2;
          }
          local8 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local8);
          local11 = local6 * local8 - local11 < .5 ? local11 : local6 * local8 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          local10 += 1023;
          let local12 = (local11 - local8) / local8 >= 1;
          local7 = local12 ? local10 + 1 : local10;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local12 ? 0 : local11 - local8;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          local4.push(Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local18 = 0;
      for (; local18 < 8; local18++) {
        output[++local0] = local4[local18] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
      return local0;
    } else {
      local0 = position;
      let local4 = Number(position) + 1;
      output[local4] = mask() ^ 128;
      let local5;
      if (floatScratch) {
        floatScratch[0] = value;
        local5 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local5 = nanData;
      } else {
        let local6 = value < 0 || value === 0 && 1 / value < 0;
        let local7 = Math.abs(value);
        let local8, local9;
        if (local7 >= Math.pow(2, -1022)) {
          let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
          local7 = local7 / Math.pow(2, local10);
          let local11 = local7 < 1;
          local7 = local11 ? local7 * 2 : local7;
          local11 = local11 ? local10 - 1 : local10;
          if (local7 >= 2) {
            ++local11;
            local7 = local7 / 2;
          }
          let local12 = Math.pow(2, 52);
          local7 = local7 * local12;
          let local13 = Math.floor(local7);
          if (!(local7 - local13 < .5)) if (local7 - local13 > .5) {
            ++local13;
          } else {
            local13 = [local13, local13 % 2];
            local13 = local13[1] ? local13[0] + 1 : local13[0];
          }
          local13 = local13 - local12;
          local11 += 1023;
          local12 = local13 / local12 >= 1;
          local9 = local12 ? local11 + 1 : local11;
          if (local9 > 2046) {
            local8 = 0;
            local9 = 2047;
          } else {
            local8 = local12 ? 0 : local13;
          }
        } else {
          local7 = local7 / Math.pow(2, -1074);
          local8 = Math.floor(local7);
          if (!(local7 - local8 < .5)) if (local7 - local8 > .5) {
            ++local8;
          } else {
            local8 = [local8, local8 % 2];
            local8 = local8[1] ? local8[0] + 1 : local8[0];
          }
          local9 = 0;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local9 & 1));
          local9 = Math.floor(local9 / 2);
        }
        local14.unshift(local6);
        local5 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          let local18 = [local5, local5.push];
          local18[1].call(local18[0], Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local4] = local5[local19] ^ mask();
      }
      return local4;
    }
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      let local4 = Number(position) + 1;
      output[local4] = mask() ^ 128;
      let local5;
      if (floatScratch) {
        floatScratch[0] = value;
        local5 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local5 = nanData;
      } else {
        const local6 = value < 0 || value === 0 && 1 / value < 0;
        let local7 = Math.abs(value);
        let local8, local9;
        if (local7 >= Math.pow(2, -1022)) {
          let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
          local7 = local7 / Math.pow(2, local10);
          let local11 = local7 < 1;
          local10 = local11 ? local10 - 1 : local10;
          local11 = local11 ? local7 * 2 : local7;
          let local12 = local11 >= 2;
          let local13 = Math.pow(2, 52);
          let local14 = Math.floor((local12 ? local11 / 2 : local11) * local13);
          if (!((local12 ? local11 / 2 : local11) * local13 - local14 < .5)) if ((local12 ? local11 / 2 : local11) * local13 - local14 > .5) {
            ++local14;
          } else {
            local14 = [local14, local14 % 2];
            local14 = local14[1] ? local14[0] + 1 : local14[0];
          }
          local12 = (local12 ? local10 + 1 : local10) + 1023;
          local9 = (local14 - local13) / local13 >= 1;
          local8 = local9 ? local12 + 1 : local12;
          local9 = local9 ? 0 : local14 - local13;
          if (local8 > 2046) {
            local8 = 2047;
            local9 = 0;
          }
        } else {
          local7 = local7 / Math.pow(2, -1074);
          local9 = Math.floor(local7);
          if (!(local7 - local9 < .5)) if (local7 - local9 > .5) {
            ++local9;
          } else {
            local9 = [local9, local9 % 2];
            local9 = local9[1] ? local9[0] + 1 : local9[0];
          }
          local8 = 0;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local9 & 1));
          local9 = Math.floor(local9 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local15.unshift(local6);
        local5 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          local5.push(Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local4] = local5[local19] ^ mask();
      }
      local0 = local4;
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local6 = local10 ? local6 * 2 : local6;
          local10 = local10 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local10;
            local6 = local6 / 2;
          }
          let local11 = Math.pow(2, 52);
          let local12 = Math.floor(local6 * local11);
          const local13 = local6 * local11 - local12 < .5 ? local12 : local6 * local11 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12;
          local10 += 1023;
          let local14 = (local13 - local11) / local11 >= 1;
          local8 = local14 ? local10 + 1 : local10;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local14 ? 0 : local13 - local11;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local15 = Math.floor(local6);
          local7 = local6 - local15 < .5 ? local15 : local6 - local15 > .5 ? local15 + 1 : local15 % 2 ? local15 + 1 : local15;
          local8 = 0;
        }
        let local16 = [];
        let local17 = 51;
        for (; local17 >= 0; local17--) {
          local16.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local18 = 11;
        for (; local18 > 0; local18--) {
          local16.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local16.unshift(local5);
        local4 = [];
        let local19 = 56;
        for (; local19 >= 0; local19 = local19 - 8) {
          local4.push(Number(local16[local19]) << 7 | Number(local16[local19 + 1]) << 6 | Number(local16[local19 + 2]) << 5 | Number(local16[local19 + 3]) << 4 | Number(local16[local19 + 4]) << 3 | Number(local16[local19 + 5]) << 2 | Number(local16[local19 + 6]) << 1 | Number(local16[local19 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local0] = local4[local20] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local8 = local6 < 1;
          local6 = local8 ? local6 * 2 : local6;
          local8 = local8 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local8;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local10);
          local6 = local6 * local10;
          if (!(local6 - local11 < .5)) if (local6 - local11 > .5) {
            ++local11;
          } else {
            local11 = [local11, local11 % 2];
            local11 = local11[1] ? local11[0] + 1 : local11[0];
          }
          local11 = local11 - local10;
          local10 = local11 / local10 >= 1;
          local8 = local10 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local10 ? 0 : local11;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          local7 = Math.floor(local6);
          if (!(local6 - local7 < .5)) if (local6 - local7 > .5) {
            ++local7;
          } else {
            local7 = [local7, local7 % 2];
            local7 = local7[1] ? local7[0] + 1 : local7[0];
          }
          local8 = 0;
        }
        let local12 = [];
        let local13 = 51;
        for (; local13 >= 0; local13--) {
          local12.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local14 = 11;
        for (; local14 > 0; local14--) {
          local12.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local12.unshift(local5);
        local4 = [];
        let local15 = 56;
        for (; local15 >= 0; local15 = local15 - 8) {
          local4.push(Number(local12[local15]) << 7 | Number(local12[local15 + 1]) << 6 | Number(local12[local15 + 2]) << 5 | Number(local12[local15 + 3]) << 4 | Number(local12[local15 + 4]) << 3 | Number(local12[local15 + 5]) << 2 | Number(local12[local15 + 6]) << 1 | Number(local12[local15 + 7]));
        }
      }
      let local16 = 0;
      for (; local16 < 8; local16++) {
        output[++local0] = local4[local16] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          local7 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local7);
          let local9 = local6 < 1;
          local7 = local9 ? local7 - 1 : local7;
          local9 = local9 ? local6 * 2 : local6;
          let local10 = local9 >= 2;
          local7 = local10 ? local7 + 1 : local7;
          local10 = local10 ? local9 / 2 : local9;
          let local11 = Math.pow(2, 52);
          local8 = Math.floor(local10 * local11);
          local8 = local10 * local11 - local8 < .5 ? local8 : local10 * local11 - local8 > .5 ? local8 + 1 : local8 % 2 ? local8 + 1 : local8;
          let local12 = (local8 - local11) / local11 >= 1;
          local7 = local12 ? local7 + 1024 : local7 + 1023;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local12 ? 0 : local8 - local11;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          let local18 = [local4, local4.push];
          local18[1].call(local18[0], Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local0] = local4[local19] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local9 = local10 ? local9 - 1 : local9;
          local10 = local10 ? local6 * 2 : local6;
          let local11 = local10 >= 2;
          local10 = local11 ? local10 / 2 : local10;
          local8 = Math.pow(2, 52);
          let local12 = Math.floor(local10 * local8);
          local10 = local10 * local8 - local12 < .5 ? local12 : local10 * local8 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12;
          local11 = (local11 ? local9 + 1 : local9) + 1023;
          let local13 = (local10 - local8) / local8 >= 1;
          local7 = local13 ? local11 + 1 : local11;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local13 ? 0 : local10 - local8;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local14 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local14 < .5 ? local14 : local6 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local15.unshift(local5);
        local4 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          local4.push(Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local0] = local4[local19] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local2 | (local3 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local5] = (local3 >> 5 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local6] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[local6 + 1] = local3 >>> 19 ^ mask();
        local0 = local6 + 1;
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local7] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 19 & 127 | 128) ^ mask();
        local0 = local7 + 1;
        output[local0] = local3 >>> 26 ^ mask();
      }
    } else {
      let local8 = output;
      local0 = Number(position) + 1;
      local8[local0] = mask() ^ 128;
      let local9, local10;
      if (floatScratch) {
        floatScratch[0] = value;
        local9 = new byteConstructor(floatScratch.buffer);
      } else {
        let local11;
        if (value !== value) {
          local11 = nanData;
        } else {
          let local12 = value < 0 || value === 0 && 1 / value < 0;
          let local13 = Math.abs(value);
          let local14, local15;
          if (local13 >= Math.pow(2, -1022)) {
            let local16 = Math.min(Math.floor(Math.log(local13) / Math.LN2), 1023);
            local13 = local13 / Math.pow(2, local16);
            let local17 = local13 < 1;
            local13 = local17 ? local13 * 2 : local13;
            local17 = local17 ? local16 - 1 : local16;
            if (local13 >= 2) {
              ++local17;
              local13 = local13 / 2;
            }
            let local18 = Math.pow(2, 52);
            local13 = local13 * local18;
            let local19 = Math.floor(local13);
            local17 += 1023;
            let local20 = ((local13 - local19 < .5 ? local19 : local13 - local19 > .5 ? local19 + 1 : local19 % 2 ? local19 + 1 : local19) - local18) / local18 >= 1;
            local15 = local20 ? local17 + 1 : local17;
            if (local15 > 2046) {
              local14 = 0;
              local15 = 2047;
            } else {
              local14 = local20 ? 0 : (local13 - local19 < .5 ? local19 : local13 - local19 > .5 ? local19 + 1 : local19 % 2 ? local19 + 1 : local19) - local18;
            }
          } else {
            local13 = local13 / Math.pow(2, -1074);
            let local21 = Math.floor(local13);
            local14 = local13 - local21 < .5 ? local21 : local13 - local21 > .5 ? local21 + 1 : local21 % 2 ? local21 + 1 : local21;
            local15 = 0;
          }
          let local22 = [];
          let local23 = 51;
          for (; local23 >= 0; local23--) {
            local22.unshift(!!(local14 & 1));
            local14 = Math.floor(local14 / 2);
          }
          let local24 = 11;
          for (; local24 > 0; local24--) {
            local22.unshift(!!(local15 & 1));
            local15 = Math.floor(local15 / 2);
          }
          local22.unshift(local12);
          let local25 = [];
          let local26 = 56;
          while (local26 >= 0) {
            let local27 = [local25, local25.push];
            local27[1].call(local27[0], Number(local22[local26]) << 7 | Number(local22[local26 + 1]) << 6 | Number(local22[local26 + 2]) << 5 | Number(local22[local26 + 3]) << 4 | Number(local22[local26 + 4]) << 3 | Number(local22[local26 + 5]) << 2 | Number(local22[local26 + 6]) << 1 | Number(local22[local26 + 7]));
            local26 = local26 - 8;
          }
          local11 = local25;
        }
        local9 = local11;
      }
      local10 = output;
      let local28 = 0;
      for (; local28 < 8; local28++) {
        local10[++local0] = local9[local28] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        const local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local8 = local6 < 1;
          local6 = local8 ? local6 * 2 : local6;
          local8 = local8 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local8;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          local6 = local6 * local10;
          let local11 = Math.floor(local6);
          local6 = local6 - local11 < .5 ? local11 : local6 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          let local12 = (local6 - local10) / local10 >= 1;
          local8 = local12 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local12 ? 0 : local6 - local10;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
          local8 = 0;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          local4.push(Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local18 = 0;
      for (; local18 < 8; local18++) {
        output[++local0] = local4[local18] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
      return local0;
    } else {
      local0 = position;
      let local4 = Number(position) + 1;
      output[local4] = mask() ^ 128;
      let local5;
      if (floatScratch) {
        floatScratch[0] = value;
        local5 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local5 = nanData;
      } else {
        let local6 = value < 0 || value === 0 && 1 / value < 0;
        let local7 = Math.abs(value);
        let local8, local9;
        if (local7 >= Math.pow(2, -1022)) {
          let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
          local7 = local7 / Math.pow(2, local10);
          let local11 = local7 < 1;
          local7 = local11 ? local7 * 2 : local7;
          local11 = local11 ? local10 - 1 : local10;
          if (local7 >= 2) {
            ++local11;
            local7 = local7 / 2;
          }
          let local12 = Math.pow(2, 52);
          let local13 = Math.floor(local7 * local12);
          local7 = local7 * local12;
          local11 += 1023;
          let local14 = ((local7 - local13 < .5 ? local13 : local7 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13) - local12) / local12 >= 1;
          local9 = local14 ? local11 + 1 : local11;
          if (local9 > 2046) {
            local8 = 0;
            local9 = 2047;
          } else {
            local8 = local14 ? 0 : (local7 - local13 < .5 ? local13 : local7 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13) - local12;
          }
        } else {
          local7 = local7 / Math.pow(2, -1074);
          let local15 = Math.floor(local7);
          local8 = local7 - local15 < .5 ? local15 : local7 - local15 > .5 ? local15 + 1 : local15 % 2 ? local15 + 1 : local15;
          local9 = 0;
        }
        let local16 = [];
        let local17 = 51;
        for (; local17 >= 0; local17--) {
          local16.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local18 = 11;
        for (; local18 > 0; local18--) {
          local16.unshift(!!(local9 & 1));
          local9 = Math.floor(local9 / 2);
        }
        local16.unshift(local6);
        local5 = [];
        let local19 = 56;
        for (; local19 >= 0; local19 = local19 - 8) {
          local5.push(Number(local16[local19]) << 7 | Number(local16[local19 + 1]) << 6 | Number(local16[local19 + 2]) << 5 | Number(local16[local19 + 3]) << 4 | Number(local16[local19 + 4]) << 3 | Number(local16[local19 + 5]) << 2 | Number(local16[local19 + 6]) << 1 | Number(local16[local19 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local4] = local5[local20] ^ mask();
      }
      return local4;
    }
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local9 = local10 ? local9 - 1 : local9;
          local10 = local10 ? local6 * 2 : local6;
          let local11 = local10 >= 2;
          let local12 = Math.pow(2, 52);
          let local13 = Math.floor((local11 ? local10 / 2 : local10) * local12);
          if (!((local11 ? local10 / 2 : local10) * local12 - local13 < .5)) if ((local11 ? local10 / 2 : local10) * local12 - local13 > .5) {
            ++local13;
          } else {
            local13 = [local13, local13 % 2];
            local13 = local13[1] ? local13[0] + 1 : local13[0];
          }
          local11 = (local11 ? local9 + 1 : local9) + 1023;
          local8 = (local13 - local12) / local12 >= 1;
          local7 = local8 ? local11 + 1 : local11;
          local8 = local8 ? 0 : local13 - local12;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          local8 = Math.floor(local6);
          if (!(local6 - local8 < .5)) if (local6 - local8 > .5) {
            ++local8;
          } else {
            local8 = [local8, local8 % 2];
            local8 = local8[1] ? local8[0] + 1 : local8[0];
          }
          local7 = 0;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        for (let local17 = 56; local17 >= 0; local17 = local17 - 8) {
          local4.push(Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local18 = 0;
      for (; local18 < 8; local18++) {
        output[++local0] = local4[local18] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        let local4 = Number(position) + 1;
        output[local4] = (local3 | (local2 & 31 | 32)) ^ mask();
        local0 = local4;
        local0 = Number(local4) + 1;
        output[local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local5;
      if (floatScratch) {
        floatScratch[0] = value;
        local5 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local5 = nanData;
      } else {
        let local6 = value < 0 || value === 0 && 1 / value < 0;
        let local7 = Math.abs(value);
        let local8, local9;
        if (local7 >= Math.pow(2, -1022)) {
          let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
          local7 = local7 / Math.pow(2, local10);
          let local11 = local7 < 1;
          local7 = local11 ? local7 * 2 : local7;
          local11 = local11 ? local10 - 1 : local10;
          if (local7 >= 2) {
            ++local11;
            local7 = local7 / 2;
          }
          local8 = Math.pow(2, 52);
          let local12 = Math.floor(local7 * local8);
          local11 += 1023;
          local9 = ((local7 * local8 - local12 < .5 ? local12 : local7 * local8 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12) - local8) / local8 >= 1;
          local8 = local9 ? 0 : (local7 * local8 - local12 < .5 ? local12 : local7 * local8 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12) - local8;
          local9 = local9 ? local11 + 1 : local11;
          if (local9 > 2046) {
            local8 = 0;
            local9 = 2047;
          }
        } else {
          local7 = local7 / Math.pow(2, -1074);
          let local13 = Math.floor(local7);
          local8 = local7 - local13 < .5 ? local13 : local7 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
          local9 = 0;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local9 & 1));
          local9 = Math.floor(local9 / 2);
        }
        local14.unshift(local6);
        let local17 = [];
        let local18 = 56;
        let local19, local20;
        for (;; local18 = local20 - 8) {
          local19 = local17;
          local20 = local18;
          if (!(local18 >= 0)) break;
          local19.push(Number(local14[local20]) << 7 | Number(local14[local20 + 1]) << 6 | Number(local14[local20 + 2]) << 5 | Number(local14[local20 + 3]) << 4 | Number(local14[local20 + 4]) << 3 | Number(local14[local20 + 5]) << 2 | Number(local14[local20 + 6]) << 1 | Number(local14[local20 + 7]));
        }
        local5 = local19;
      }
      let local21 = 0;
      for (; local21 < 8; local21++) {
        output[++local0] = local5[local21] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local3 | (local2 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local5] = (local2 >> 5 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local6] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local2 >>> 12 & 127 | 128) ^ mask();
        local0 = local6 + 1;
        output[local0] = local2 >>> 19 ^ mask();
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local7] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local7] = (local2 >>> 19 & 127 | 128) ^ mask();
        local0 = local7 + 1;
        output[local0] = local2 >>> 26 ^ mask();
      }
    } else {
      let local8 = output;
      local0 = Number(position) + 1;
      local8[local0] = mask() ^ 128;
      let local9;
      if (floatScratch) {
        floatScratch[0] = value;
        local9 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local9 = nanData;
      } else {
        let local10 = value < 0 || value === 0 && 1 / value < 0;
        let local11 = Math.abs(value);
        let local12, local13;
        if (local11 >= Math.pow(2, -1022)) {
          let local14 = Math.min(Math.floor(Math.log(local11) / Math.LN2), 1023);
          local11 = local11 / Math.pow(2, local14);
          local13 = local11 < 1;
          local11 = local13 ? local11 * 2 : local11;
          local13 = local13 ? local14 - 1 : local14;
          if (local11 >= 2) {
            ++local13;
            local11 = local11 / 2;
          }
          let local15 = Math.pow(2, 52);
          let local16 = Math.floor(local11 * local15);
          local11 = local11 * local15;
          local11 = local11 - local16 < .5 ? local16 : local11 - local16 > .5 ? local16 + 1 : local16 % 2 ? local16 + 1 : local16;
          let local17 = (local11 - local15) / local15 >= 1;
          local13 = local17 ? local13 + 1024 : local13 + 1023;
          if (local13 > 2046) {
            local12 = 0;
            local13 = 2047;
          } else {
            local12 = local17 ? 0 : local11 - local15;
          }
        } else {
          local11 = local11 / Math.pow(2, -1074);
          let local18 = Math.floor(local11);
          local12 = local11 - local18 < .5 ? local18 : local11 - local18 > .5 ? local18 + 1 : local18 % 2 ? local18 + 1 : local18;
          local13 = 0;
        }
        let local19 = [];
        let local20 = 51;
        for (; local20 >= 0; local20--) {
          local19.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        let local21 = 11;
        for (; local21 > 0; local21--) {
          local19.unshift(!!(local13 & 1));
          local13 = Math.floor(local13 / 2);
        }
        local19.unshift(local10);
        local9 = [];
        let local22 = 56;
        for (; local22 >= 0; local22 = local22 - 8) {
          let local23 = [local9, local9.push];
          local23[1].call(local23[0], Number(local19[local22]) << 7 | Number(local19[local22 + 1]) << 6 | Number(local19[local22 + 2]) << 5 | Number(local19[local22 + 3]) << 4 | Number(local19[local22 + 4]) << 3 | Number(local19[local22 + 5]) << 2 | Number(local19[local22 + 6]) << 1 | Number(local19[local22 + 7]));
        }
      }
      let local24 = output;
      let local25 = 0;
      for (; local25 < 8; local25++) {
        local24[++local0] = local9[local25] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local3 | (local2 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local5] = (local2 >> 5 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local6] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local2 >>> 12 & 127 | 128) ^ mask();
        local0 = local6 + 1;
        output[local0] = local2 >>> 19 ^ mask();
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local7] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local7] = (local2 >>> 19 & 127 | 128) ^ mask();
        local0 = local7 + 1;
        output[local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local8;
      if (floatScratch) {
        floatScratch[0] = value;
        local8 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local8 = nanData;
      } else {
        let local9 = value < 0 || value === 0 && 1 / value < 0;
        let local10 = Math.abs(value);
        let local11, local12;
        if (local10 >= Math.pow(2, -1022)) {
          local11 = Math.min(Math.floor(Math.log(local10) / Math.LN2), 1023);
          local10 = local10 / Math.pow(2, local11);
          let local13 = local10 < 1;
          local11 = local13 ? local11 - 1 : local11;
          local13 = local13 ? local10 * 2 : local10;
          let local14 = local13 >= 2;
          local11 = local14 ? local11 + 1 : local11;
          let local15 = Math.pow(2, 52);
          let local16 = Math.floor((local14 ? local13 / 2 : local13) * local15);
          if (!((local14 ? local13 / 2 : local13) * local15 - local16 < .5)) if ((local14 ? local13 / 2 : local13) * local15 - local16 > .5) {
            ++local16;
          } else {
            local16 = [local16, local16 % 2];
            local16 = local16[1] ? local16[0] + 1 : local16[0];
          }
          local12 = (local16 - local15) / local15 >= 1;
          local11 = local12 ? local11 + 1024 : local11 + 1023;
          local12 = local12 ? 0 : local16 - local15;
          if (local11 > 2046) {
            local11 = 2047;
            local12 = 0;
          }
        } else {
          local10 = local10 / Math.pow(2, -1074);
          local12 = Math.floor(local10);
          if (!(local10 - local12 < .5)) if (local10 - local12 > .5) {
            ++local12;
          } else {
            local12 = [local12, local12 % 2];
            local12 = local12[1] ? local12[0] + 1 : local12[0];
          }
          local11 = 0;
        }
        let local17 = [];
        let local18 = 51;
        for (; local18 >= 0; local18--) {
          local17.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        let local19 = 11;
        for (; local19 > 0; local19--) {
          local17.unshift(!!(local11 & 1));
          local11 = Math.floor(local11 / 2);
        }
        local17.unshift(local9);
        local8 = [];
        let local20 = 56;
        for (; local20 >= 0; local20 = local20 - 8) {
          let local21 = [local8, local8.push];
          local21[1].call(local21[0], Number(local17[local20]) << 7 | Number(local17[local20 + 1]) << 6 | Number(local17[local20 + 2]) << 5 | Number(local17[local20 + 3]) << 4 | Number(local17[local20 + 4]) << 3 | Number(local17[local20 + 5]) << 2 | Number(local17[local20 + 6]) << 1 | Number(local17[local20 + 7]));
        }
      }
      let local22 = 0;
      for (; local22 < 8; local22++) {
        output[++local0] = local8[local22] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local2 | (local3 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[local5 + 1] = (local3 >> 5 & 127 | 128) ^ mask();
        output[local5 + 2] = local3 >>> 12 ^ mask();
        local0 = local5 + 2;
      } else if (local3 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local6] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[local6 + 1] = local3 >>> 19 ^ mask();
        local0 = local6 + 1;
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local7] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[local7 + 1] = local3 >>> 26 ^ mask();
        local0 = local7 + 1;
      }
    } else {
      let local8 = output;
      local0 = Number(position) + 1;
      local8[local0] = mask() ^ 128;
      let local9;
      if (floatScratch) {
        floatScratch[0] = value;
        local9 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local9 = nanData;
      } else {
        let local10 = value < 0 || value === 0 && 1 / value < 0;
        let local11 = Math.abs(value);
        let local12, local13;
        if (local11 >= Math.pow(2, -1022)) {
          let local14 = Math.min(Math.floor(Math.log(local11) / Math.LN2), 1023);
          local11 = local11 / Math.pow(2, local14);
          let local15 = local11 < 1;
          local14 = local15 ? local14 - 1 : local14;
          local15 = local15 ? local11 * 2 : local11;
          let local16 = local15 >= 2;
          local15 = local16 ? local15 / 2 : local15;
          local13 = Math.pow(2, 52);
          let local17 = Math.floor(local15 * local13);
          local15 = local15 * local13 - local17 < .5 ? local17 : local15 * local13 - local17 > .5 ? local17 + 1 : local17 % 2 ? local17 + 1 : local17;
          local16 = (local16 ? local14 + 1 : local14) + 1023;
          let local18 = (local15 - local13) / local13 >= 1;
          local12 = local18 ? local16 + 1 : local16;
          if (local12 > 2046) {
            local12 = 2047;
            local13 = 0;
          } else {
            local13 = local18 ? 0 : local15 - local13;
          }
        } else {
          local11 = local11 / Math.pow(2, -1074);
          let local19 = Math.floor(local11);
          local12 = 0;
          local13 = local11 - local19 < .5 ? local19 : local11 - local19 > .5 ? local19 + 1 : local19 % 2 ? local19 + 1 : local19;
        }
        let local20 = [];
        let local21 = 51;
        for (; local21 >= 0; local21--) {
          local20.unshift(!!(local13 & 1));
          local13 = Math.floor(local13 / 2);
        }
        let local22 = 11;
        for (; local22 > 0; local22--) {
          local20.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        local20.unshift(local10);
        local9 = [];
        let local23 = 56;
        for (; local23 >= 0; local23 = local23 - 8) {
          local9.push(Number(local20[local23]) << 7 | Number(local20[local23 + 1]) << 6 | Number(local20[local23 + 2]) << 5 | Number(local20[local23 + 3]) << 4 | Number(local20[local23 + 4]) << 3 | Number(local20[local23 + 5]) << 2 | Number(local20[local23 + 6]) << 1 | Number(local20[local23 + 7]));
        }
      }
      let local24 = output;
      let local25 = 0;
      for (; local25 < 8; local25++) {
        local24[++local0] = local9[local25] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
      return local0;
    } else {
      local0 = position;
      let local4 = Number(position) + 1;
      output[local4] = mask() ^ 128;
      let local5;
      if (floatScratch) {
        floatScratch[0] = value;
        local5 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local5 = nanData;
      } else {
        let local6 = value < 0 || value === 0 && 1 / value < 0;
        let local7 = Math.abs(value);
        let local8, local9;
        if (local7 >= Math.pow(2, -1022)) {
          let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
          local7 = local7 / Math.pow(2, local10);
          let local11 = local7 < 1;
          local10 = local11 ? local10 - 1 : local10;
          local11 = local11 ? local7 * 2 : local7;
          let local12 = local11 >= 2;
          local10 = local12 ? local10 + 1 : local10;
          let local13 = Math.pow(2, 52);
          let local14 = Math.floor((local12 ? local11 / 2 : local11) * local13);
          if (!((local12 ? local11 / 2 : local11) * local13 - local14 < .5)) if ((local12 ? local11 / 2 : local11) * local13 - local14 > .5) {
            ++local14;
          } else {
            local14 = [local14, local14 % 2];
            local14 = local14[1] ? local14[0] + 1 : local14[0];
          }
          local9 = (local14 - local13) / local13 >= 1;
          local13 = local9 ? 0 : local14 - local13;
          local9 = local9 ? local10 + 1024 : local10 + 1023;
          if (local9 > 2046) {
            local8 = 0;
            local9 = 2047;
          } else {
            local8 = local13;
          }
        } else {
          local7 = local7 / Math.pow(2, -1074);
          local8 = Math.floor(local7);
          if (!(local7 - local8 < .5)) if (local7 - local8 > .5) {
            ++local8;
          } else {
            local8 = [local8, local8 % 2];
            local8 = local8[1] ? local8[0] + 1 : local8[0];
          }
          local9 = 0;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local9 & 1));
          local9 = Math.floor(local9 / 2);
        }
        local15.unshift(local6);
        local5 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          local5.push(Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local4] = local5[local19] ^ mask();
      }
      return local4;
    }
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local6 = local10 ? local6 * 2 : local6;
          local10 = local10 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local10;
            local6 = local6 / 2;
          }
          let local11 = Math.pow(2, 52);
          let local12 = Math.floor(local6 * local11);
          local12 = (local6 * local11 - local12 < .5 ? local12 : local6 * local11 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12) - local11;
          local10 += 1023;
          local11 = local12 / local11 >= 1;
          local8 = local11 ? local10 + 1 : local10;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local11 ? 0 : local12;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
          local8 = 0;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          let local18 = [local4, local4.push];
          local18[1].call(local18[0], Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local0] = local4[local19] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local9 = local10 ? local9 - 1 : local9;
          local10 = local10 ? local6 * 2 : local6;
          let local11 = local10 >= 2;
          local10 = local11 ? local10 / 2 : local10;
          local8 = Math.pow(2, 52);
          let local12 = Math.floor(local10 * local8);
          local11 = (local11 ? local9 + 1 : local9) + 1023;
          let local13 = ((local10 * local8 - local12 < .5 ? local12 : local10 * local8 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12) - local8) / local8 >= 1;
          local7 = local13 ? local11 + 1 : local11;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local13 ? 0 : (local10 * local8 - local12 < .5 ? local12 : local10 * local8 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12) - local8;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local14 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local14 < .5 ? local14 : local6 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local15.unshift(local5);
        local4 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          let local19 = [local4, local4.push];
          local19[1].call(local19[0], Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local0] = local4[local20] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local6 = local10 ? local6 * 2 : local6;
          local10 = local10 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local10;
            local6 = local6 / 2;
          }
          local7 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local7);
          if (!(local6 * local7 - local11 < .5)) if (local6 * local7 - local11 > .5) {
            ++local11;
          } else {
            local11 = [local11, local11 % 2];
            local11 = local11[1] ? local11[0] + 1 : local11[0];
          }
          local8 = (local11 - local7) / local7 >= 1;
          local7 = local8 ? 0 : local11 - local7;
          local8 = local8 ? local10 + 1024 : local10 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          local7 = Math.floor(local6);
          if (!(local6 - local7 < .5)) if (local6 - local7 > .5) {
            ++local7;
          } else {
            local7 = [local7, local7 % 2];
            local7 = local7[1] ? local7[0] + 1 : local7[0];
          }
          local8 = 0;
        }
        let local12 = [];
        let local13 = 51;
        for (; local13 >= 0; local13--) {
          local12.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local14 = 11;
        for (; local14 > 0; local14--) {
          local12.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local12.unshift(local5);
        local4 = [];
        let local15 = 56;
        for (; local15 >= 0; local15 = local15 - 8) {
          local4.push(Number(local12[local15]) << 7 | Number(local12[local15 + 1]) << 6 | Number(local12[local15 + 2]) << 5 | Number(local12[local15 + 3]) << 4 | Number(local12[local15 + 4]) << 3 | Number(local12[local15 + 5]) << 2 | Number(local12[local15 + 6]) << 1 | Number(local12[local15 + 7]));
        }
      }
      let local16 = 0;
      for (; local16 < 8; local16++) {
        output[++local0] = local4[local16] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        const local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local8 = local6 < 1;
          local6 = local8 ? local6 * 2 : local6;
          local8 = local8 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local8;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local10);
          const local12 = local6 * local10 - local11 < .5 ? local11 : local6 * local10 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          let local13 = (local12 - local10) / local10 >= 1;
          local8 = local13 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local13 ? 0 : local12 - local10;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local14 = Math.floor(local6);
          local7 = local6 - local14 < .5 ? local14 : local6 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
          local8 = 0;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local15.unshift(local5);
        local4 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          let local19 = [local4, local4.push];
          local19[1].call(local19[0], Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local0] = local4[local20] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          local8 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local8);
          let local9 = local6 < 1;
          local8 = local9 ? local8 - 1 : local8;
          local9 = local9 ? local6 * 2 : local6;
          let local10 = local9 >= 2;
          local8 = local10 ? local8 + 1 : local8;
          let local11 = Math.pow(2, 52);
          let local12 = Math.floor((local10 ? local9 / 2 : local9) * local11);
          local10 = (local10 ? local9 / 2 : local9) * local11 - local12 < .5 ? local12 : (local10 ? local9 / 2 : local9) * local11 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12;
          let local13 = (local10 - local11) / local11 >= 1;
          local8 = local13 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local13 ? 0 : local10 - local11;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local14 = Math.floor(local6);
          local7 = local6 - local14 < .5 ? local14 : local6 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
          local8 = 0;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local15.unshift(local5);
        local4 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          local4.push(Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local0] = local4[local19] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[local4 + 1] = local2 >> 5 ^ mask();
        local0 = local4 + 1;
      } else if (local2 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[local5 + 1] = (local2 >> 5 & 127 | 128) ^ mask();
        output[local5 + 2] = local2 >>> 12 ^ mask();
        local0 = local5 + 2;
      } else if (local2 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[local6 + 1] = (local2 >> 5 & 127 | 128) ^ mask();
        output[local6 + 2] = (local2 >>> 12 & 127 | 128) ^ mask();
        local0 = local6 + 3;
        output[local0] = local2 >>> 19 ^ mask();
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local7] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[local7 + 1] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[local7 + 2] = local2 >>> 26 ^ mask();
        local0 = local7 + 2;
      }
    } else {
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local8;
      if (floatScratch) {
        floatScratch[0] = value;
        local8 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local8 = nanData;
      } else {
        let local9 = value < 0 || value === 0 && 1 / value < 0;
        let local10 = Math.abs(value);
        let local11, local12;
        if (local10 >= Math.pow(2, -1022)) {
          let local13 = Math.min(Math.floor(Math.log(local10) / Math.LN2), 1023);
          local10 = local10 / Math.pow(2, local13);
          let local14 = local10 < 1;
          local13 = local14 ? local13 - 1 : local13;
          local14 = local14 ? local10 * 2 : local10;
          let local15 = local14 >= 2;
          let local16 = Math.pow(2, 52);
          let local17 = Math.floor((local15 ? local14 / 2 : local14) * local16);
          if (!((local15 ? local14 / 2 : local14) * local16 - local17 < .5)) if ((local15 ? local14 / 2 : local14) * local16 - local17 > .5) {
            ++local17;
          } else {
            local17 = [local17, local17 % 2];
            local17 = local17[1] ? local17[0] + 1 : local17[0];
          }
          local15 = (local15 ? local13 + 1 : local13) + 1023;
          local12 = (local17 - local16) / local16 >= 1;
          local11 = local12 ? local15 + 1 : local15;
          local12 = local12 ? 0 : local17 - local16;
          if (local11 > 2046) {
            local11 = 2047;
            local12 = 0;
          }
        } else {
          local10 = local10 / Math.pow(2, -1074);
          local12 = Math.floor(local10);
          if (!(local10 - local12 < .5)) if (local10 - local12 > .5) {
            ++local12;
          } else {
            local12 = [local12, local12 % 2];
            local12 = local12[1] ? local12[0] + 1 : local12[0];
          }
          local11 = 0;
        }
        let local18 = [];
        let local19 = 51;
        for (; local19 >= 0; local19--) {
          local18.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        let local20 = 11;
        for (; local20 > 0; local20--) {
          local18.unshift(!!(local11 & 1));
          local11 = Math.floor(local11 / 2);
        }
        local18.unshift(local9);
        local8 = [];
        let local21 = 56;
        for (; local21 >= 0; local21 = local21 - 8) {
          local8.push(Number(local18[local21]) << 7 | Number(local18[local21 + 1]) << 6 | Number(local18[local21 + 2]) << 5 | Number(local18[local21 + 3]) << 4 | Number(local18[local21 + 4]) << 3 | Number(local18[local21 + 5]) << 2 | Number(local18[local21 + 6]) << 1 | Number(local18[local21 + 7]));
        }
      }
      let local22 = output;
      let local23 = 0;
      for (; local23 < 8; local23++) {
        local22[++local0] = local8[local23] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      let local4 = Number(position) + 1;
      output[local4] = mask() ^ 128;
      let local5;
      if (floatScratch) {
        floatScratch[0] = value;
        local5 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local5 = nanData;
      } else {
        let local6 = value < 0 || value === 0 && 1 / value < 0;
        let local7 = Math.abs(value);
        let local8, local9;
        if (local7 >= Math.pow(2, -1022)) {
          let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
          local7 = local7 / Math.pow(2, local10);
          let local11 = local7 < 1;
          local7 = local11 ? local7 * 2 : local7;
          local11 = local11 ? local10 - 1 : local10;
          if (local7 >= 2) {
            ++local11;
            local7 = local7 / 2;
          }
          let local12 = Math.pow(2, 52);
          let local13 = Math.floor(local7 * local12);
          local7 = local7 * local12;
          local7 = local7 - local13 < .5 ? local13 : local7 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
          local11 += 1023;
          let local14 = (local7 - local12) / local12 >= 1;
          local9 = local14 ? local11 + 1 : local11;
          if (local9 > 2046) {
            local8 = 0;
            local9 = 2047;
          } else {
            local8 = local14 ? 0 : local7 - local12;
          }
        } else {
          local7 = local7 / Math.pow(2, -1074);
          let local15 = Math.floor(local7);
          local8 = local7 - local15 < .5 ? local15 : local7 - local15 > .5 ? local15 + 1 : local15 % 2 ? local15 + 1 : local15;
          local9 = 0;
        }
        let local16 = [];
        let local17 = 51;
        for (; local17 >= 0; local17--) {
          local16.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local18 = 11;
        for (; local18 > 0; local18--) {
          local16.unshift(!!(local9 & 1));
          local9 = Math.floor(local9 / 2);
        }
        local16.unshift(local6);
        local5 = [];
        let local19 = 56;
        for (; local19 >= 0; local19 = local19 - 8) {
          local5.push(Number(local16[local19]) << 7 | Number(local16[local19 + 1]) << 6 | Number(local16[local19 + 2]) << 5 | Number(local16[local19 + 3]) << 4 | Number(local16[local19 + 4]) << 3 | Number(local16[local19 + 5]) << 2 | Number(local16[local19 + 6]) << 1 | Number(local16[local19 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local4] = local5[local20] ^ mask();
      }
      local0 = local4;
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local8 = local6 < 1;
          local6 = local8 ? local6 * 2 : local6;
          local8 = local8 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local8;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local10);
          if (!(local6 * local10 - local11 < .5)) {
            local11 = local6 * local10 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          }
          local11 = local11 - local10;
          local10 = local11 / local10 >= 1;
          local8 = local10 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local10 ? 0 : local11;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local12 = Math.floor(local6);
          local7 = local6 - local12 < .5 ? local12 : local6 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12;
          local8 = 0;
        }
        let local13 = [];
        let local14 = 51;
        for (; local14 >= 0; local14--) {
          local13.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local15 = 11;
        for (; local15 > 0; local15--) {
          local13.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local13.unshift(local5);
        local4 = [];
        let local16 = 56;
        for (; local16 >= 0; local16 = local16 - 8) {
          local4.push(Number(local13[local16]) << 7 | Number(local13[local16 + 1]) << 6 | Number(local13[local16 + 2]) << 5 | Number(local13[local16 + 3]) << 4 | Number(local13[local16 + 4]) << 3 | Number(local13[local16 + 5]) << 2 | Number(local13[local16 + 6]) << 1 | Number(local13[local16 + 7]));
        }
      }
      let local17 = 0;
      for (; local17 < 8; local17++) {
        output[++local0] = local4[local17] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        const local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local7 = local6 < 1;
          local6 = local7 ? local6 * 2 : local6;
          local7 = local7 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local7;
            local6 = local6 / 2;
          }
          local8 = Math.pow(2, 52);
          let local10 = Math.floor(local6 * local8);
          local10 = (local6 * local8 - local10 < .5 ? local10 : local6 * local8 - local10 > .5 ? local10 + 1 : local10 % 2 ? local10 + 1 : local10) - local8;
          local8 = local10 / local8 >= 1;
          local7 = local8 ? local7 + 1024 : local7 + 1023;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local8 ? 0 : local10;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local11 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local11 < .5 ? local11 : local6 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
        }
        let local12 = [];
        let local13 = 51;
        for (; local13 >= 0; local13--) {
          local12.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local14 = 11;
        for (; local14 > 0; local14--) {
          local12.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local12.unshift(local5);
        local4 = [];
        let local15 = 56;
        for (; local15 >= 0; local15 = local15 - 8) {
          let local16 = [local4, local4.push];
          local16[1].call(local16[0], Number(local12[local15]) << 7 | Number(local12[local15 + 1]) << 6 | Number(local12[local15 + 2]) << 5 | Number(local12[local15 + 3]) << 4 | Number(local12[local15 + 4]) << 3 | Number(local12[local15 + 5]) << 2 | Number(local12[local15 + 6]) << 1 | Number(local12[local15 + 7]));
        }
      }
      let local17 = 0;
      for (; local17 < 8; local17++) {
        output[++local0] = local4[local17] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local2 | (local3 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local5] = (local3 >> 5 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local6] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local3 >>> 12 & 127 | 128) ^ mask();
        local0 = local6 + 1;
        output[local0] = local3 >>> 19 ^ mask();
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local7] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 19 & 127 | 128) ^ mask();
        local0 = local7 + 1;
        output[local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local8;
      if (floatScratch) {
        floatScratch[0] = value;
        local8 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local8 = nanData;
      } else {
        const local9 = value < 0 || value === 0 && 1 / value < 0;
        let local10 = Math.abs(value);
        let local11;
        if (local10 >= Math.pow(2, -1022)) {
          let local12 = Math.min(Math.floor(Math.log(local10) / Math.LN2), 1023);
          local10 = local10 / Math.pow(2, local12);
          if (local10 < 1) {
            --local12;
            local10 = local10 * 2;
          }
          let local13 = local10 >= 2;
          local12 = local13 ? local12 + 1 : local12;
          local13 = local13 ? local10 / 2 : local10;
          let local14 = Math.pow(2, 52);
          let local15 = Math.floor(local13 * local14);
          local15 = local13 * local14 - local15 < .5 ? local15 : local13 * local14 - local15 > .5 ? local15 + 1 : local15 % 2 ? local15 + 1 : local15;
          let local16 = (local15 - local14) / local14 >= 1;
          local11 = local16 ? local12 + 1024 : local12 + 1023;
          if (local11 > 2046) {
            local11 = 2047;
            local10 = 0;
          } else {
            local10 = local16 ? 0 : local15 - local14;
          }
        } else {
          local10 = local10 / Math.pow(2, -1074);
          let local17 = Math.floor(local10);
          local11 = 0;
          local10 = local10 - local17 < .5 ? local17 : local10 - local17 > .5 ? local17 + 1 : local17 % 2 ? local17 + 1 : local17;
        }
        let local18 = [];
        let local19 = 51;
        for (; local19 >= 0; local19--) {
          local18.unshift(!!(local10 & 1));
          local10 = Math.floor(local10 / 2);
        }
        let local20 = 11;
        for (; local20 > 0; local20--) {
          local18.unshift(!!(local11 & 1));
          local11 = Math.floor(local11 / 2);
        }
        local18.unshift(local9);
        local8 = [];
        let local21 = 56;
        for (; local21 >= 0; local21 = local21 - 8) {
          local8.push(Number(local18[local21]) << 7 | Number(local18[local21 + 1]) << 6 | Number(local18[local21 + 2]) << 5 | Number(local18[local21 + 3]) << 4 | Number(local18[local21 + 4]) << 3 | Number(local18[local21 + 5]) << 2 | Number(local18[local21 + 6]) << 1 | Number(local18[local21 + 7]));
        }
      }
      let local22 = output;
      let local23 = 0;
      for (; local23 < 8; local23++) {
        local22[++local0] = local8[local23] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local2 | (local3 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local5] = (local3 >> 5 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[local6 + 1] = (local3 >> 5 & 127 | 128) ^ mask();
        output[local6 + 2] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[local6 + 3] = local3 >>> 19 ^ mask();
        local0 = local6 + 3;
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local7] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[local7 + 1] = local3 >>> 26 ^ mask();
        local0 = local7 + 1;
      }
    } else {
      let local8 = output;
      local0 = Number(position) + 1;
      local8[local0] = mask() ^ 128;
      let local9;
      if (floatScratch) {
        floatScratch[0] = value;
        local9 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local9 = nanData;
      } else {
        const local10 = value < 0 || value === 0 && 1 / value < 0;
        let local11 = Math.abs(value);
        let local12, local13;
        if (local11 >= Math.pow(2, -1022)) {
          local12 = Math.min(Math.floor(Math.log(local11) / Math.LN2), 1023);
          local11 = local11 / Math.pow(2, local12);
          let local14 = local11 < 1;
          local12 = local14 ? local12 - 1 : local12;
          local14 = local14 ? local11 * 2 : local11;
          let local15 = local14 >= 2;
          local12 = local15 ? local12 + 1 : local12;
          let local16 = Math.pow(2, 52);
          let local17 = Math.floor((local15 ? local14 / 2 : local14) * local16);
          if (!((local15 ? local14 / 2 : local14) * local16 - local17 < .5)) if ((local15 ? local14 / 2 : local14) * local16 - local17 > .5) {
            ++local17;
          } else {
            local17 = [local17, local17 % 2];
            local17 = local17[1] ? local17[0] + 1 : local17[0];
          }
          local13 = local17 - local16;
          local16 = local13 / local16 >= 1;
          local12 = local16 ? local12 + 1024 : local12 + 1023;
          if (local12 > 2046) {
            local12 = 2047;
            local13 = 0;
          } else {
            local13 = local16 ? 0 : local13;
          }
        } else {
          local11 = local11 / Math.pow(2, -1074);
          local13 = Math.floor(local11);
          if (!(local11 - local13 < .5)) if (local11 - local13 > .5) {
            ++local13;
          } else {
            local13 = [local13, local13 % 2];
            local13 = local13[1] ? local13[0] + 1 : local13[0];
          }
          local12 = 0;
        }
        let local18 = [];
        let local19 = 51;
        for (; local19 >= 0; local19--) {
          local18.unshift(!!(local13 & 1));
          local13 = Math.floor(local13 / 2);
        }
        let local20 = 11;
        for (; local20 > 0; local20--) {
          local18.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        local18.unshift(local10);
        local9 = [];
        let local21 = 56;
        for (; local21 >= 0; local21 = local21 - 8) {
          local9.push(Number(local18[local21]) << 7 | Number(local18[local21 + 1]) << 6 | Number(local18[local21 + 2]) << 5 | Number(local18[local21 + 3]) << 4 | Number(local18[local21 + 4]) << 3 | Number(local18[local21 + 5]) << 2 | Number(local18[local21 + 6]) << 1 | Number(local18[local21 + 7]));
        }
      }
      let local22 = output;
      let local23 = 0;
      for (; local23 < 8; local23++) {
        local22[++local0] = local9[local23] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          if (local6 < 1) {
            local6 = local6 * 2;
            --local9;
          }
          if (local6 >= 2) {
            ++local9;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local10);
          const local12 = local6 * local10 - local11 < .5 ? local11 : local6 * local10 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          local9 += 1023;
          let local13 = (local12 - local10) / local10 >= 1;
          local7 = local13 ? local9 + 1 : local9;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local13 ? 0 : local12 - local10;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local14 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local14 < .5 ? local14 : local6 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local15.unshift(local5);
        local4 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          let local19 = [local4, local4.push];
          local19[1].call(local19[0], Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local0] = local4[local20] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else {
        let local5;
        if (value !== value) {
          local5 = nanData;
        } else {
          let local6 = value < 0 || value === 0 && 1 / value < 0;
          let local7 = Math.abs(value);
          let local8, local9;
          if (local7 >= Math.pow(2, -1022)) {
            let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
            local7 = local7 / Math.pow(2, local10);
            let local11 = local7 < 1;
            local10 = local11 ? local10 - 1 : local10;
            local11 = local11 ? local7 * 2 : local7;
            let local12 = local11 >= 2;
            local11 = local12 ? local11 / 2 : local11;
            local9 = Math.pow(2, 52);
            let local13 = Math.floor(local11 * local9);
            local11 = local11 * local9 - local13 < .5 ? local13 : local11 * local9 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
            local12 = (local12 ? local10 + 1 : local10) + 1023;
            let local14 = (local11 - local9) / local9 >= 1;
            local8 = local14 ? local12 + 1 : local12;
            if (local8 > 2046) {
              local8 = 2047;
              local9 = 0;
            } else {
              local9 = local14 ? 0 : local11 - local9;
            }
          } else {
            local7 = local7 / Math.pow(2, -1074);
            let local15 = Math.floor(local7);
            local8 = 0;
            local9 = local7 - local15 < .5 ? local15 : local7 - local15 > .5 ? local15 + 1 : local15 % 2 ? local15 + 1 : local15;
          }
          let local16 = [];
          let local17 = 51;
          for (; local17 >= 0; local17--) {
            local16.unshift(!!(local9 & 1));
            local9 = Math.floor(local9 / 2);
          }
          let local18 = 11;
          for (; local18 > 0; local18--) {
            local16.unshift(!!(local8 & 1));
            local8 = Math.floor(local8 / 2);
          }
          local16.unshift(local6);
          let local19 = [];
          let local20 = 56;
          while (local20 >= 0) {
            let local21 = [local19, local19.push];
            local21[1].call(local21[0], Number(local16[local20]) << 7 | Number(local16[local20 + 1]) << 6 | Number(local16[local20 + 2]) << 5 | Number(local16[local20 + 3]) << 4 | Number(local16[local20 + 4]) << 3 | Number(local16[local20 + 5]) << 2 | Number(local16[local20 + 6]) << 1 | Number(local16[local20 + 7]));
            local20 = local20 - 8;
          }
          local5 = local19;
        }
        local4 = local5;
      }
      let local22 = 0;
      for (; local22 < 8; local22++) {
        output[++local0] = local4[local22] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local6 = local10 ? local6 * 2 : local6;
          local10 = local10 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local10;
            local6 = local6 / 2;
          }
          let local11 = Math.pow(2, 52);
          local6 = local6 * local11;
          let local12 = Math.floor(local6);
          if (!(local6 - local12 < .5)) if (local6 - local12 > .5) {
            ++local12;
          } else {
            local12 = [local12, local12 % 2];
            local12 = local12[1] ? local12[0] + 1 : local12[0];
          }
          local10 += 1023;
          local8 = (local12 - local11) / local11 >= 1;
          local11 = local8 ? 0 : local12 - local11;
          local8 = local8 ? local10 + 1 : local10;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local11;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          local7 = Math.floor(local6);
          if (!(local6 - local7 < .5)) if (local6 - local7 > .5) {
            ++local7;
          } else {
            local7 = [local7, local7 % 2];
            local7 = local7[1] ? local7[0] + 1 : local7[0];
          }
          local8 = 0;
        }
        let local13 = [];
        let local14 = 51;
        for (; local14 >= 0; local14--) {
          local13.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local15 = 11;
        for (; local15 > 0; local15--) {
          local13.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local13.unshift(local5);
        local4 = [];
        let local16 = 56;
        for (; local16 >= 0; local16 = local16 - 8) {
          local4.push(Number(local13[local16]) << 7 | Number(local13[local16 + 1]) << 6 | Number(local13[local16 + 2]) << 5 | Number(local13[local16 + 3]) << 4 | Number(local13[local16 + 4]) << 3 | Number(local13[local16 + 5]) << 2 | Number(local13[local16 + 6]) << 1 | Number(local13[local16 + 7]));
        }
      }
      let local17 = 0;
      for (; local17 < 8; local17++) {
        output[++local0] = local4[local17] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          local7 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local7);
          let local9 = local6 < 1;
          local7 = local9 ? local7 - 1 : local7;
          local9 = local9 ? local6 * 2 : local6;
          let local10 = local9 >= 2;
          local7 = local10 ? local7 + 1 : local7;
          local10 = local10 ? local9 / 2 : local9;
          local8 = Math.pow(2, 52);
          let local11 = Math.floor(local10 * local8);
          local11 = local10 * local8 - local11 < .5 ? local11 : local10 * local8 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          let local12 = (local11 - local8) / local8 >= 1;
          local7 = local12 ? local7 + 1024 : local7 + 1023;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local12 ? 0 : local11 - local8;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          local4.push(Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local18 = 0;
      for (; local18 < 8; local18++) {
        output[++local0] = local4[local18] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local8 = local6 < 1;
          local6 = local8 ? local6 * 2 : local6;
          local8 = local8 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local8;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          local6 = local6 * local10;
          let local11 = Math.floor(local6);
          let local12 = ((local6 - local11 < .5 ? local11 : local6 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11) - local10) / local10 >= 1;
          local8 = local12 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local12 ? 0 : (local6 - local11 < .5 ? local11 : local6 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11) - local10;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
          local8 = 0;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          local4.push(Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local18 = 0;
      for (; local18 < 8; local18++) {
        output[++local0] = local4[local18] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          local7 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local7);
          let local9 = local6 < 1;
          local7 = local9 ? local7 - 1 : local7;
          local9 = local9 ? local6 * 2 : local6;
          let local10 = local9 >= 2;
          local7 = local10 ? local7 + 1 : local7;
          local8 = Math.pow(2, 52);
          let local11 = Math.floor((local10 ? local9 / 2 : local9) * local8);
          local10 = (local10 ? local9 / 2 : local9) * local8 - local11 < .5 ? local11 : (local10 ? local9 / 2 : local9) * local8 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          let local12 = (local10 - local8) / local8 >= 1;
          local7 = local12 ? local7 + 1024 : local7 + 1023;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local12 ? 0 : local10 - local8;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          let local18 = [local4, local4.push];
          local18[1].call(local18[0], Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local0] = local4[local19] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local2 | (local3 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local5] = (local3 >> 5 & 127 | 128) ^ mask();
        output[local5 + 1] = local3 >>> 12 ^ mask();
        local0 = local5 + 1;
      } else if (local3 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local6] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local3 >>> 12 & 127 | 128) ^ mask();
        local0 = local6 + 1;
        output[local0] = local3 >>> 19 ^ mask();
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local7] = (local3 >> 5 & 127 | 128) ^ mask();
        output[local7 + 1] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[local7 + 2] = (local3 >>> 19 & 127 | 128) ^ mask();
        local0 = local7 + 3;
        output[local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local8;
      if (floatScratch) {
        floatScratch[0] = value;
        local8 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local8 = nanData;
      } else {
        let local9 = value < 0 || value === 0 && 1 / value < 0;
        let local10 = Math.abs(value);
        let local11, local12;
        if (local10 >= Math.pow(2, -1022)) {
          let local13 = Math.min(Math.floor(Math.log(local10) / Math.LN2), 1023);
          local10 = local10 / Math.pow(2, local13);
          let local14 = local10 < 1;
          local10 = local14 ? local10 * 2 : local10;
          local14 = local14 ? local13 - 1 : local13;
          if (local10 >= 2) {
            ++local14;
            local10 = local10 / 2;
          }
          let local15 = Math.pow(2, 52);
          let local16 = Math.floor(local10 * local15);
          local16 = local10 * local15 - local16 < .5 ? local16 : local10 * local15 - local16 > .5 ? local16 + 1 : local16 % 2 ? local16 + 1 : local16;
          local14 += 1023;
          let local17 = (local16 - local15) / local15 >= 1;
          local12 = local17 ? local14 + 1 : local14;
          if (local12 > 2046) {
            local11 = 0;
            local12 = 2047;
          } else {
            local11 = local17 ? 0 : local16 - local15;
          }
        } else {
          local10 = local10 / Math.pow(2, -1074);
          let local18 = Math.floor(local10);
          local11 = local10 - local18 < .5 ? local18 : local10 - local18 > .5 ? local18 + 1 : local18 % 2 ? local18 + 1 : local18;
          local12 = 0;
        }
        let local19 = [];
        let local20 = 51;
        for (; local20 >= 0; local20--) {
          local19.unshift(!!(local11 & 1));
          local11 = Math.floor(local11 / 2);
        }
        let local21 = 11;
        for (; local21 > 0; local21--) {
          local19.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        local19.unshift(local9);
        local8 = [];
        let local22 = 56;
        for (; local22 >= 0; local22 = local22 - 8) {
          local8.push(Number(local19[local22]) << 7 | Number(local19[local22 + 1]) << 6 | Number(local19[local22 + 2]) << 5 | Number(local19[local22 + 3]) << 4 | Number(local19[local22 + 4]) << 3 | Number(local19[local22 + 5]) << 2 | Number(local19[local22 + 6]) << 1 | Number(local19[local22 + 7]));
        }
      }
      let local23 = 0;
      for (; local23 < 8; local23++) {
        output[++local0] = local8[local23] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local6 = local10 ? local6 * 2 : local6;
          local10 = local10 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local10;
            local6 = local6 / 2;
          }
          local7 = Math.pow(2, 52);
          local6 = local6 * local7;
          let local11 = Math.floor(local6);
          if (!(local6 - local11 < .5)) if (local6 - local11 > .5) {
            ++local11;
          } else {
            local11 = [local11, local11 % 2];
            local11 = local11[1] ? local11[0] + 1 : local11[0];
          }
          local8 = (local11 - local7) / local7 >= 1;
          local7 = local8 ? 0 : local11 - local7;
          local8 = local8 ? local10 + 1024 : local10 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          local7 = Math.floor(local6);
          if (!(local6 - local7 < .5)) if (local6 - local7 > .5) {
            ++local7;
          } else {
            local7 = [local7, local7 % 2];
            local7 = local7[1] ? local7[0] + 1 : local7[0];
          }
          local8 = 0;
        }
        let local12 = [];
        let local13 = 51;
        for (; local13 >= 0; local13--) {
          local12.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local14 = 11;
        for (; local14 > 0; local14--) {
          local12.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local12.unshift(local5);
        local4 = [];
        let local15 = 56;
        for (; local15 >= 0; local15 = local15 - 8) {
          let local16 = [local4, local4.push];
          local16[1].call(local16[0], Number(local12[local15]) << 7 | Number(local12[local15 + 1]) << 6 | Number(local12[local15 + 2]) << 5 | Number(local12[local15 + 3]) << 4 | Number(local12[local15 + 4]) << 3 | Number(local12[local15 + 5]) << 2 | Number(local12[local15 + 6]) << 1 | Number(local12[local15 + 7]));
        }
      }
      let local17 = 0;
      for (; local17 < 8; local17++) {
        output[++local0] = local4[local17] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local9 = local10 ? local9 - 1 : local9;
          local10 = local10 ? local6 * 2 : local6;
          let local11 = local10 >= 2;
          local10 = local11 ? local10 / 2 : local10;
          local8 = Math.pow(2, 52);
          let local12 = Math.floor(local10 * local8);
          local10 = local10 * local8 - local12 < .5 ? local12 : local10 * local8 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12;
          local11 = (local11 ? local9 + 1 : local9) + 1023;
          let local13 = (local10 - local8) / local8 >= 1;
          local7 = local13 ? local11 + 1 : local11;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local13 ? 0 : local10 - local8;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local14 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local14 < .5 ? local14 : local6 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local15.unshift(local5);
        local4 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          local4.push(Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local0] = local4[local19] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        const local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local6 = local10 ? local6 * 2 : local6;
          local10 = local10 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local10;
            local6 = local6 / 2;
          }
          local7 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local7);
          if (!(local6 * local7 - local11 < .5)) if (local6 * local7 - local11 > .5) {
            ++local11;
          } else {
            local11 = [local11, local11 % 2];
            local11 = local11[1] ? local11[0] + 1 : local11[0];
          }
          local8 = (local11 - local7) / local7 >= 1;
          local7 = local8 ? 0 : local11 - local7;
          local8 = local8 ? local10 + 1024 : local10 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          local7 = Math.floor(local6);
          if (!(local6 - local7 < .5)) if (local6 - local7 > .5) {
            ++local7;
          } else {
            local7 = [local7, local7 % 2];
            local7 = local7[1] ? local7[0] + 1 : local7[0];
          }
          local8 = 0;
        }
        let local12 = [];
        let local13 = 51;
        for (; local13 >= 0; local13--) {
          local12.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local14 = 11;
        for (; local14 > 0; local14--) {
          local12.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local12.unshift(local5);
        local4 = [];
        let local15 = 56;
        for (; local15 >= 0; local15 = local15 - 8) {
          local4.push(Number(local12[local15]) << 7 | Number(local12[local15 + 1]) << 6 | Number(local12[local15 + 2]) << 5 | Number(local12[local15 + 3]) << 4 | Number(local12[local15 + 4]) << 3 | Number(local12[local15 + 5]) << 2 | Number(local12[local15 + 6]) << 1 | Number(local12[local15 + 7]));
        }
      }
      let local16 = 0;
      for (; local16 < 8; local16++) {
        output[++local0] = local4[local16] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
      return local0;
    } else {
      local0 = position;
      let local4 = Number(position) + 1;
      output[local4] = mask() ^ 128;
      let local5;
      if (floatScratch) {
        floatScratch[0] = value;
        local5 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local5 = nanData;
      } else {
        let local6 = value < 0 || value === 0 && 1 / value < 0;
        let local7 = Math.abs(value);
        let local8, local9;
        if (local7 >= Math.pow(2, -1022)) {
          let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
          local7 = local7 / Math.pow(2, local10);
          let local11 = local7 < 1;
          local10 = local11 ? local10 - 1 : local10;
          local11 = local11 ? local7 * 2 : local7;
          let local12 = local11 >= 2;
          local11 = local12 ? local11 / 2 : local11;
          let local13 = Math.pow(2, 52);
          let local14 = Math.floor(local11 * local13);
          local12 = (local12 ? local10 + 1 : local10) + 1023;
          let local15 = ((local11 * local13 - local14 < .5 ? local14 : local11 * local13 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14) - local13) / local13 >= 1;
          local9 = local15 ? local12 + 1 : local12;
          if (local9 > 2046) {
            local8 = 0;
            local9 = 2047;
          } else {
            local8 = local15 ? 0 : (local11 * local13 - local14 < .5 ? local14 : local11 * local13 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14) - local13;
          }
        } else {
          local7 = local7 / Math.pow(2, -1074);
          let local16 = Math.floor(local7);
          local8 = local7 - local16 < .5 ? local16 : local7 - local16 > .5 ? local16 + 1 : local16 % 2 ? local16 + 1 : local16;
          local9 = 0;
        }
        let local17 = [];
        let local18 = 51;
        for (; local18 >= 0; local18--) {
          local17.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local19 = 11;
        for (; local19 > 0; local19--) {
          local17.unshift(!!(local9 & 1));
          local9 = Math.floor(local9 / 2);
        }
        local17.unshift(local6);
        local5 = [];
        let local20 = 56;
        for (; local20 >= 0; local20 = local20 - 8) {
          local5.push(Number(local17[local20]) << 7 | Number(local17[local20 + 1]) << 6 | Number(local17[local20 + 2]) << 5 | Number(local17[local20 + 3]) << 4 | Number(local17[local20 + 4]) << 3 | Number(local17[local20 + 5]) << 2 | Number(local17[local20 + 6]) << 1 | Number(local17[local20 + 7]));
        }
      }
      let local21 = 0;
      for (; local21 < 8; local21++) {
        output[++local4] = local5[local21] ^ mask();
      }
      return local4;
    }
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local2 | (local3 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local5] = (local3 >> 5 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local6] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local3 >>> 12 & 127 | 128) ^ mask();
        local0 = local6 + 1;
        output[local0] = local3 >>> 19 ^ mask();
      } else {
        let local7 = Number(position) + 1;
        output[local7] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local7] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local7] = (local3 >>> 19 & 127 | 128) ^ mask();
        local0 = local7 + 1;
        output[local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local8;
      if (floatScratch) {
        floatScratch[0] = value;
        local8 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local8 = nanData;
      } else {
        let local9 = value < 0 || value === 0 && 1 / value < 0;
        let local10 = Math.abs(value);
        let local11, local12;
        if (local10 >= Math.pow(2, -1022)) {
          let local13 = Math.min(Math.floor(Math.log(local10) / Math.LN2), 1023);
          local10 = local10 / Math.pow(2, local13);
          local12 = local10 < 1;
          local10 = local12 ? local10 * 2 : local10;
          local12 = local12 ? local13 - 1 : local13;
          if (local10 >= 2) {
            ++local12;
            local10 = local10 / 2;
          }
          let local14 = Math.pow(2, 52);
          let local15 = Math.floor(local10 * local14);
          const local16 = local10 * local14 - local15 < .5 ? local15 : local10 * local14 - local15 > .5 ? local15 + 1 : local15 % 2 ? local15 + 1 : local15;
          let local17 = (local16 - local14) / local14 >= 1;
          local12 = local17 ? local12 + 1024 : local12 + 1023;
          if (local12 > 2046) {
            local11 = 0;
            local12 = 2047;
          } else {
            local11 = local17 ? 0 : local16 - local14;
          }
        } else {
          local10 = local10 / Math.pow(2, -1074);
          let local18 = Math.floor(local10);
          local11 = local10 - local18 < .5 ? local18 : local10 - local18 > .5 ? local18 + 1 : local18 % 2 ? local18 + 1 : local18;
          local12 = 0;
        }
        let local19 = [];
        let local20 = 51;
        for (; local20 >= 0; local20--) {
          local19.unshift(!!(local11 & 1));
          local11 = Math.floor(local11 / 2);
        }
        let local21 = 11;
        for (; local21 > 0; local21--) {
          local19.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        local19.unshift(local9);
        local8 = [];
        let local22 = 56;
        for (; local22 >= 0; local22 = local22 - 8) {
          local8.push(Number(local19[local22]) << 7 | Number(local19[local22 + 1]) << 6 | Number(local19[local22 + 2]) << 5 | Number(local19[local22 + 3]) << 4 | Number(local19[local22 + 4]) << 3 | Number(local19[local22 + 5]) << 2 | Number(local19[local22 + 6]) << 1 | Number(local19[local22 + 7]));
        }
      }
      let local23 = 0;
      for (; local23 < 8; local23++) {
        output[++local0] = local8[local23] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local3 | (local2 & 31 | 32)) ^ mask();
        local4 = [local4, output];
        local0 = local4[0] + 1;
        local4[1][local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local5] = (local2 >> 5 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local6] = (local2 >> 5 & 127 | 128) ^ mask();
        local6 = [local6, output];
        let local7 = local6[0] + 1;
        local6[1][local7] = (local2 >>> 12 & 127 | 128) ^ mask();
        local7 = [local7, output];
        local0 = local7[0] + 1;
        local7[1][local0] = local2 >>> 19 ^ mask();
      } else {
        let local8 = Number(position) + 1;
        output[local8] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local8] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local8] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local8] = (local2 >>> 19 & 127 | 128) ^ mask();
        local8 = [local8, output];
        local0 = local8[0] + 1;
        local8[1][local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local9;
      if (floatScratch) {
        floatScratch[0] = value;
        local9 = new byteConstructor(floatScratch.buffer);
      } else {
        let local10;
        if (value !== value) {
          local10 = nanData;
        } else {
          let local11 = value < 0 || value === 0 && 1 / value < 0;
          let local12 = Math.abs(value);
          let local13, local14;
          if (local12 >= Math.pow(2, -1022)) {
            let local15 = Math.min(Math.floor(Math.log(local12) / Math.LN2), 1023);
            local12 = local12 / Math.pow(2, local15);
            let local16 = local12 < 1;
            local12 = local16 ? local12 * 2 : local12;
            local16 = local16 ? local15 - 1 : local15;
            if (local12 >= 2) {
              ++local16;
              local12 = local12 / 2;
            }
            let local17 = Math.pow(2, 52);
            let local18 = Math.floor(local12 * local17);
            local18 = local12 * local17 - local18 < .5 ? local18 : local12 * local17 - local18 > .5 ? local18 + 1 : local18 % 2 ? local18 + 1 : local18;
            local16 += 1023;
            let local19 = (local18 - local17) / local17 >= 1;
            local14 = local19 ? local16 + 1 : local16;
            if (local14 > 2046) {
              local13 = 0;
              local14 = 2047;
            } else {
              local13 = local19 ? 0 : local18 - local17;
            }
          } else {
            local12 = local12 / Math.pow(2, -1074);
            let local20 = Math.floor(local12);
            local13 = local12 - local20 < .5 ? local20 : local12 - local20 > .5 ? local20 + 1 : local20 % 2 ? local20 + 1 : local20;
            local14 = 0;
          }
          let local21 = [];
          let local22 = 51;
          for (; local22 >= 0; local22--) {
            local21.unshift(!!(local13 & 1));
            local13 = Math.floor(local13 / 2);
          }
          let local23 = 11;
          for (; local23 > 0; local23--) {
            local21.unshift(!!(local14 & 1));
            local14 = Math.floor(local14 / 2);
          }
          local21.unshift(local11);
          let local24 = [];
          let local25 = 56;
          while (local25 >= 0) {
            let local26 = [local24, local24.push];
            local26[1].call(local26[0], Number(local21[local25]) << 7 | Number(local21[local25 + 1]) << 6 | Number(local21[local25 + 2]) << 5 | Number(local21[local25 + 3]) << 4 | Number(local21[local25 + 4]) << 3 | Number(local21[local25 + 5]) << 2 | Number(local21[local25 + 6]) << 1 | Number(local21[local25 + 7]));
            local25 = local25 - 8;
          }
          local10 = local24;
        }
        local9 = local10;
      }
      let local27 = 0;
      for (; local27 < 8; local27++) {
        output[++local0] = local9[local27] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        const local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local8 = local6 < 1;
          local6 = local8 ? local6 * 2 : local6;
          local8 = local8 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local8;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          local6 = local6 * local10;
          let local11 = Math.floor(local6);
          local6 = local6 - local11 < .5 ? local11 : local6 - local11 > .5 ? local11 + 1 : local11 % 2 ? local11 + 1 : local11;
          let local12 = (local6 - local10) / local10 >= 1;
          local8 = local12 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local12 ? 0 : local6 - local10;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local13 = Math.floor(local6);
          local7 = local6 - local13 < .5 ? local13 : local6 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
          local8 = 0;
        }
        let local14 = [];
        let local15 = 51;
        for (; local15 >= 0; local15--) {
          local14.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local16 = 11;
        for (; local16 > 0; local16--) {
          local14.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local14.unshift(local5);
        local4 = [];
        let local17 = 56;
        for (; local17 >= 0; local17 = local17 - 8) {
          local4.push(Number(local14[local17]) << 7 | Number(local14[local17 + 1]) << 6 | Number(local14[local17 + 2]) << 5 | Number(local14[local17 + 3]) << 4 | Number(local14[local17 + 4]) << 3 | Number(local14[local17 + 5]) << 2 | Number(local14[local17 + 6]) << 1 | Number(local14[local17 + 7]));
        }
      }
      let local18 = 0;
      for (; local18 < 8; local18++) {
        output[++local0] = local4[local18] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[local4 + 1] = local2 >> 5 ^ mask();
        local0 = local4 + 1;
      } else if (local2 < 524288) {
        let local5 = Number(position) + 1;
        output[local5] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[local5 + 1] = (local2 >> 5 & 127 | 128) ^ mask();
        output[local5 + 2] = local2 >>> 12 ^ mask();
        local0 = local5 + 2;
      } else if (local2 < 67108864) {
        let local6 = Number(position) + 1;
        output[local6] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[local6 + 1] = (local2 >> 5 & 127 | 128) ^ mask();
        output[local6 + 2] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[local6 + 3] = local2 >>> 19 ^ mask();
        local0 = local6 + 3;
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      let local7 = Number(position) + 1;
      output[local7] = mask() ^ 128;
      let local8;
      if (floatScratch) {
        floatScratch[0] = value;
        local8 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local8 = nanData;
      } else {
        const local9 = value < 0 || value === 0 && 1 / value < 0;
        let local10 = Math.abs(value);
        let local11, local12;
        if (local10 >= Math.pow(2, -1022)) {
          let local13 = Math.min(Math.floor(Math.log(local10) / Math.LN2), 1023);
          local10 = local10 / Math.pow(2, local13);
          let local14 = local10 < 1;
          local10 = local14 ? local10 * 2 : local10;
          local14 = local14 ? local13 - 1 : local13;
          if (local10 >= 2) {
            ++local14;
            local10 = local10 / 2;
          }
          local11 = Math.pow(2, 52);
          let local15 = Math.floor(local10 * local11);
          if (!(local10 * local11 - local15 < .5)) if (local10 * local11 - local15 > .5) {
            ++local15;
          } else {
            local15 = [local15, local15 % 2];
            local15 = local15[1] ? local15[0] + 1 : local15[0];
          }
          local12 = (local15 - local11) / local11 >= 1;
          local11 = local12 ? 0 : local15 - local11;
          local12 = local12 ? local14 + 1024 : local14 + 1023;
          if (local12 > 2046) {
            local11 = 0;
            local12 = 2047;
          }
        } else {
          local10 = local10 / Math.pow(2, -1074);
          local11 = Math.floor(local10);
          if (!(local10 - local11 < .5)) if (local10 - local11 > .5) {
            ++local11;
          } else {
            local11 = [local11, local11 % 2];
            local11 = local11[1] ? local11[0] + 1 : local11[0];
          }
          local12 = 0;
        }
        let local16 = [];
        let local17 = 51;
        for (; local17 >= 0; local17--) {
          local16.unshift(!!(local11 & 1));
          local11 = Math.floor(local11 / 2);
        }
        let local18 = 11;
        for (; local18 > 0; local18--) {
          local16.unshift(!!(local12 & 1));
          local12 = Math.floor(local12 / 2);
        }
        local16.unshift(local9);
        local8 = [];
        let local19 = 56;
        for (; local19 >= 0; local19 = local19 - 8) {
          local8.push(Number(local16[local19]) << 7 | Number(local16[local19 + 1]) << 6 | Number(local16[local19 + 2]) << 5 | Number(local16[local19 + 3]) << 4 | Number(local16[local19 + 4]) << 3 | Number(local16[local19 + 5]) << 2 | Number(local16[local19 + 6]) << 1 | Number(local16[local19 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local7] = local8[local20] ^ mask();
      }
      local0 = local7;
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          let local10 = local6 < 1;
          local9 = local10 ? local9 - 1 : local9;
          local10 = local10 ? local6 * 2 : local6;
          let local11 = local10 >= 2;
          local10 = local11 ? local10 / 2 : local10;
          local8 = Math.pow(2, 52);
          let local12 = Math.floor(local10 * local8);
          local10 = local10 * local8 - local12 < .5 ? local12 : local10 * local8 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12;
          local11 = (local11 ? local9 + 1 : local9) + 1023;
          let local13 = (local10 - local8) / local8 >= 1;
          local7 = local13 ? local11 + 1 : local11;
          if (local7 > 2046) {
            local7 = 2047;
            local8 = 0;
          } else {
            local8 = local13 ? 0 : local10 - local8;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          let local14 = Math.floor(local6);
          local7 = 0;
          local8 = local6 - local14 < .5 ? local14 : local6 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        local15.unshift(local5);
        local4 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          let local19 = [local4, local4.push];
          local19[1].call(local19[0], Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local20 = 0;
      for (; local20 < 8; local20++) {
        output[++local0] = local4[local20] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = false;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else {
        let local5;
        if (value !== value) {
          local5 = nanData;
        } else {
          let local6 = value < 0 || value === 0 && 1 / value < 0;
          let local7 = Math.abs(value);
          let local8;
          if (local7 >= Math.pow(2, -1022)) {
            local8 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
            local7 = local7 / Math.pow(2, local8);
            let local9 = local7 < 1;
            local8 = local9 ? local8 - 1 : local8;
            local9 = local9 ? local7 * 2 : local7;
            let local10 = local9 >= 2;
            local8 = local10 ? local8 + 1 : local8;
            let local11 = Math.pow(2, 52);
            let local12 = Math.floor((local10 ? local9 / 2 : local9) * local11);
            local10 = (local10 ? local9 / 2 : local9) * local11 - local12 < .5 ? local12 : (local10 ? local9 / 2 : local9) * local11 - local12 > .5 ? local12 + 1 : local12 % 2 ? local12 + 1 : local12;
            let local13 = (local10 - local11) / local11 >= 1;
            local8 = local13 ? local8 + 1024 : local8 + 1023;
            if (local8 > 2046) {
              local8 = 2047;
              local7 = 0;
            } else {
              local7 = local13 ? 0 : local10 - local11;
            }
          } else {
            local7 = local7 / Math.pow(2, -1074);
            let local14 = Math.floor(local7);
            local8 = 0;
            local7 = local7 - local14 < .5 ? local14 : local7 - local14 > .5 ? local14 + 1 : local14 % 2 ? local14 + 1 : local14;
          }
          let local15 = [];
          let local16 = 51;
          for (; local16 >= 0; local16--) {
            local15.unshift(!!(local7 & 1));
            local7 = Math.floor(local7 / 2);
          }
          let local17 = 11;
          for (; local17 > 0; local17--) {
            local15.unshift(!!(local8 & 1));
            local8 = Math.floor(local8 / 2);
          }
          local15.unshift(local6);
          let local18 = [];
          let local19 = 56;
          while (local19 >= 0) {
            let local20 = [local18, local18.push];
            local20[1].call(local20[0], Number(local15[local19]) << 7 | Number(local15[local19 + 1]) << 6 | Number(local15[local19 + 2]) << 5 | Number(local15[local19 + 3]) << 4 | Number(local15[local19 + 4]) << 3 | Number(local15[local19 + 5]) << 2 | Number(local15[local19 + 6]) << 1 | Number(local15[local19 + 7]));
            local19 = local19 - 8;
          }
          local5 = local18;
        }
        local4 = local5;
      }
      let local21 = 0;
      for (; local21 < 8; local21++) {
        output[++local0] = local4[local21] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local2 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local4 = nanData;
      } else {
        let local5 = value < 0 || value === 0 && 1 / value < 0;
        let local6 = Math.abs(value);
        let local7, local8;
        if (local6 >= Math.pow(2, -1022)) {
          let local9 = Math.min(Math.floor(Math.log(local6) / Math.LN2), 1023);
          local6 = local6 / Math.pow(2, local9);
          local8 = local6 < 1;
          local6 = local8 ? local6 * 2 : local6;
          local8 = local8 ? local9 - 1 : local9;
          if (local6 >= 2) {
            ++local8;
            local6 = local6 / 2;
          }
          let local10 = Math.pow(2, 52);
          let local11 = Math.floor(local6 * local10);
          local6 = local6 * local10;
          if (!(local6 - local11 < .5)) if (local6 - local11 > .5) {
            ++local11;
          } else {
            local11 = [local11, local11 % 2];
            local11 = local11[1] ? local11[0] + 1 : local11[0];
          }
          local11 = local11 - local10;
          local10 = local11 / local10 >= 1;
          local8 = local10 ? local8 + 1024 : local8 + 1023;
          if (local8 > 2046) {
            local7 = 0;
            local8 = 2047;
          } else {
            local7 = local10 ? 0 : local11;
          }
        } else {
          local6 = local6 / Math.pow(2, -1074);
          local7 = Math.floor(local6);
          if (!(local6 - local7 < .5)) if (local6 - local7 > .5) {
            ++local7;
          } else {
            local7 = [local7, local7 % 2];
            local7 = local7[1] ? local7[0] + 1 : local7[0];
          }
          local8 = 0;
        }
        let local12 = [];
        let local13 = 51;
        for (; local13 >= 0; local13--) {
          local12.unshift(!!(local7 & 1));
          local7 = Math.floor(local7 / 2);
        }
        let local14 = 11;
        for (; local14 > 0; local14--) {
          local12.unshift(!!(local8 & 1));
          local8 = Math.floor(local8 / 2);
        }
        local12.unshift(local5);
        local4 = [];
        let local15 = 56;
        for (; local15 >= 0; local15 = local15 - 8) {
          local4.push(Number(local12[local15]) << 7 | Number(local12[local15 + 1]) << 6 | Number(local12[local15 + 2]) << 5 | Number(local12[local15 + 3]) << 4 | Number(local12[local15 + 4]) << 3 | Number(local12[local15 + 5]) << 2 | Number(local12[local15 + 6]) << 1 | Number(local12[local15 + 7]));
        }
      }
      let local16 = 0;
      for (; local16 < 8; local16++) {
        output[++local0] = local4[local16] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = -value;
        local3 = 64;
      } else {
        local2 = value;
        local3 = 0;
      }
      if (local2 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | local2) ^ mask();
      } else if (local2 < 4096) {
        let local4 = Number(position) + 1;
        output[local4] = (local3 | (local2 & 31 | 32)) ^ mask();
        local0 = local4 + 1;
        output[local0] = local2 >> 5 ^ mask();
      } else if (local2 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local0] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local2 >>> 12 ^ mask();
      } else if (local2 < 67108864) {
        let local5 = Number(position) + 1;
        output[local5] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local5] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local5] = (local2 >>> 12 & 127 | 128) ^ mask();
        local0 = local5 + 1;
        output[local0] = local2 >>> 19 ^ mask();
      } else {
        let local6 = Number(position) + 1;
        output[local6] = (local3 | (local2 & 31 | 32)) ^ mask();
        output[++local6] = (local2 >> 5 & 127 | 128) ^ mask();
        output[++local6] = (local2 >>> 12 & 127 | 128) ^ mask();
        output[++local6] = (local2 >>> 19 & 127 | 128) ^ mask();
        local0 = local6 + 1;
        output[local0] = local2 >>> 26 ^ mask();
      }
    } else {
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local7;
      if (floatScratch) {
        floatScratch[0] = value;
        local7 = new byteConstructor(floatScratch.buffer);
      } else if (value !== value) {
        local7 = nanData;
      } else {
        let local8 = value < 0 || value === 0 && 1 / value < 0;
        let local9 = Math.abs(value);
        let local10, local11;
        if (local9 >= Math.pow(2, -1022)) {
          let local12 = Math.min(Math.floor(Math.log(local9) / Math.LN2), 1023);
          local9 = local9 / Math.pow(2, local12);
          local11 = local9 < 1;
          local9 = local11 ? local9 * 2 : local9;
          local11 = local11 ? local12 - 1 : local12;
          if (local9 >= 2) {
            ++local11;
            local9 = local9 / 2;
          }
          let local13 = Math.pow(2, 52);
          let local14 = Math.floor(local9 * local13);
          if (!(local9 * local13 - local14 < .5)) if (local9 * local13 - local14 > .5) {
            ++local14;
          } else {
            local14 = [local14, local14 % 2];
            local14 = local14[1] ? local14[0] + 1 : local14[0];
          }
          local14 = local14 - local13;
          local13 = local14 / local13 >= 1;
          local11 = local13 ? local11 + 1024 : local11 + 1023;
          if (local11 > 2046) {
            local10 = 0;
            local11 = 2047;
          } else {
            local10 = local13 ? 0 : local14;
          }
        } else {
          local9 = local9 / Math.pow(2, -1074);
          local10 = Math.floor(local9);
          if (!(local9 - local10 < .5)) if (local9 - local10 > .5) {
            ++local10;
          } else {
            local10 = [local10, local10 % 2];
            local10 = local10[1] ? local10[0] + 1 : local10[0];
          }
          local11 = 0;
        }
        let local15 = [];
        let local16 = 51;
        for (; local16 >= 0; local16--) {
          local15.unshift(!!(local10 & 1));
          local10 = Math.floor(local10 / 2);
        }
        let local17 = 11;
        for (; local17 > 0; local17--) {
          local15.unshift(!!(local11 & 1));
          local11 = Math.floor(local11 / 2);
        }
        local15.unshift(local8);
        local7 = [];
        let local18 = 56;
        for (; local18 >= 0; local18 = local18 - 8) {
          local7.push(Number(local15[local18]) << 7 | Number(local15[local18 + 1]) << 6 | Number(local15[local18 + 2]) << 5 | Number(local15[local18 + 3]) << 4 | Number(local15[local18 + 4]) << 3 | Number(local15[local18 + 5]) << 2 | Number(local15[local18 + 6]) << 1 | Number(local15[local18 + 7]));
        }
      }
      let local19 = 0;
      for (; local19 < 8; local19++) {
        output[++local0] = local7[local19] ^ mask();
      }
    }
    return local0;
  }"###,
r###"function numeric(mask, value, output, position) {
    let local0;
    if ((value | 0) === value) {
      let local1;
      if (value < 0) {
        local1 = value < 0;
      } else if (value === 0) {
        local1 = 1 / value < 0;
      } else {
        local1 = value === 0;
      }
      let local2, local3;
      if (local1) {
        local2 = 64;
        local3 = -value;
      } else {
        local2 = 0;
        local3 = value;
      }
      if (local3 < 32) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | local3) ^ mask();
      } else if (local3 < 4096) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = local3 >> 5 ^ mask();
      } else if (local3 < 524288) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 12 ^ mask();
      } else if (local3 < 67108864) {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 19 ^ mask();
      } else {
        local0 = Number(position) + 1;
        output[local0] = (local2 | (local3 & 31 | 32)) ^ mask();
        output[++local0] = (local3 >> 5 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 12 & 127 | 128) ^ mask();
        output[++local0] = (local3 >>> 19 & 127 | 128) ^ mask();
        output[++local0] = local3 >>> 26 ^ mask();
      }
    } else {
      local0 = position;
      local0 = Number(position) + 1;
      output[local0] = mask() ^ 128;
      let local4;
      if (floatScratch) {
        floatScratch[0] = value;
        local4 = new byteConstructor(floatScratch.buffer);
      } else {
        let local5;
        if (value !== value) {
          local5 = nanData;
        } else {
          let local6 = value < 0 || value === 0 && 1 / value < 0;
          let local7 = Math.abs(value);
          let local8, local9;
          if (local7 >= Math.pow(2, -1022)) {
            let local10 = Math.min(Math.floor(Math.log(local7) / Math.LN2), 1023);
            local7 = local7 / Math.pow(2, local10);
            let local11 = local7 < 1;
            local7 = local11 ? local7 * 2 : local7;
            local11 = local11 ? local10 - 1 : local10;
            if (local7 >= 2) {
              ++local11;
              local7 = local7 / 2;
            }
            let local12 = Math.pow(2, 52);
            local7 = local7 * local12;
            let local13 = Math.floor(local7);
            local7 = local7 - local13 < .5 ? local13 : local7 - local13 > .5 ? local13 + 1 : local13 % 2 ? local13 + 1 : local13;
            local11 += 1023;
            let local14 = (local7 - local12) / local12 >= 1;
            local9 = local14 ? local11 + 1 : local11;
            if (local9 > 2046) {
              local8 = 0;
              local9 = 2047;
            } else {
              local8 = local14 ? 0 : local7 - local12;
            }
          } else {
            local7 = local7 / Math.pow(2, -1074);
            let local15 = Math.floor(local7);
            local8 = local7 - local15 < .5 ? local15 : local7 - local15 > .5 ? local15 + 1 : local15 % 2 ? local15 + 1 : local15;
            local9 = 0;
          }
          let local16 = [];
          let local17 = 51;
          for (; local17 >= 0; local17--) {
            local16.unshift(!!(local8 & 1));
            local8 = Math.floor(local8 / 2);
          }
          let local18 = 11;
          for (; local18 > 0; local18--) {
            local16.unshift(!!(local9 & 1));
            local9 = Math.floor(local9 / 2);
          }
          local16.unshift(local6);
          let local19 = [];
          let local20 = 56;
          while (local20 >= 0) {
            local19.push(Number(local16[local20]) << 7 | Number(local16[local20 + 1]) << 6 | Number(local16[local20 + 2]) << 5 | Number(local16[local20 + 3]) << 4 | Number(local16[local20 + 4]) << 3 | Number(local16[local20 + 5]) << 2 | Number(local16[local20 + 6]) << 1 | Number(local16[local20 + 7]));
            local20 = local20 - 8;
          }
          local5 = local19;
        }
        local4 = local5;
      }
      let local21 = 0;
      for (; local21 < 8; local21++) {
        output[++local0] = local4[local21] ^ mask();
      }
    }
    return local0;
  }"###
];
