//                                                        Exact serializer reference implementation.
'use strict';
let j, prototype; let result13=Object.create(null);

//         Setup.
    function $THREAD_2_10761() {
      return record7;
    }
    function $THREAD_5_65642() {
      return result13;
    }

//    Data helpers.
    function arraysEqualByIndex(items, items2) {
      let items22 = items2;
      if (items.length === items2.length) {
        return items.every((value, value2) => items22[value2] === value);
      } else {
        return false;
      }
    }
    let encodingTags;
    function writeUnsignedIntBytes(value, items) {
      if (value <= 63) {
        items.push(value);
      } else if (value <= 16383) {
        items.push(encodingTags.UINT14_BASE | value >> 8, value & 255);
      } else if (value <= 65535) {
        items.push(encodingTags.UINT16, value >> 8, value & 255);
      } else if (value <= 16777215) {
        items.push(encodingTags.UINT24, value >> 16, value >> 8 & 255, value & 255);
      } else if (value <= 4294967295) {
        items.push(encodingTags.UINT32);
        items.push(value >>> 24, value >> 16 & 255, value >> 8 & 255, value & 255);
      } else {
        items.push(encodingTags.UINT64);
        let hash = value / 4294967296 & 4294967295;
        items.push(hash >>> 24, hash >> 16 & 255, hash >> 8 & 255, hash & 255);
        hash = value & 4294967295;
        items.push(hash >>> 24, hash >> 16 & 255, hash >> 8 & 255, hash & 255);
      }
    }

//    String table.
    let lengthByteWriter = writeUnsignedIntBytes;
    function selectStringTable(value) {
      let value2 = value;
      return Object.keys(value2).filter((items) => value2[items] >= 2 && items.length * value2[items] >= 8).map((items) => {
        return [items, (items.length + 1) * value2[items] - (items.length + 1 + value2[items] * 2)];
      }).sort((items, items2) => items2[1] - items[1]).slice(0, 255).map((items) => items[0]);
    }
    let stringTableSelector = selectStringTable;
    let arrayContentsEqual = arraysEqualByIndex;

//    False detector.
    function $THREAD_2_12162() {
      return false;
    }

//         Type tags.
    encodingTags = {
      UINT6_BASE: 0,
      UINT14_BASE: 64,
      NINT4_BASE: 128,
      BARRAY4_BASE: 144,
      ARRAY5_BASE: 160,
      STR5_BASE: 192,
      FALSE: 224,
      TRUE: 225,
      NULL: 226,
      UNDEFINED: 227,
      UINT16: 228,
      UINT24: 229,
      UINT32: 230,
      UINT64: 231,
      NINT8: 232,
      NINT16: 233,
      NINT32: 234,
      NINT64: 235,
      FLOAT32: 236,
      DOUBLE64: 237,
      TIMESTAMP: 238,
      BINARY_: 239,
      CSTRING: 240,
      STR8: 241,
      STR_: 242,
      STRREF: 243,
      ARRAY8: 244,
      ARRAY_: 245,
      BARRAY8: 246,
      BARRAY_: 247,
      MAP_: 248,
      BMAP_: 249,
      MAPL_: 250,
      BMAPL_: 251,
      STRLUT: 254,
      EXTENSION: 255
    };
    function createClassMembersInstaller() {
      function defineClassMembers(result, items, items2) {
        if (items) {
          let i = 0;
          let value, value2;
          for (; i < items.length; i++) {
            let enumerable = items[i].enumerable;
            value2 = enumerable ? value2 : (enumerable = false, value = 1, 142);
            items[i].enumerable = enumerable;
            items[i].configurable = true;
            if ("value" in items[i]) {
              items[i].writable = true;
            }
            Object.defineProperty(result.prototype, items[i].key, items[i]);
          }
        }
        if (items2) {
          for (let j = 0; j < items2.length; j++) {
            items2[j].enumerable = items2[j].enumerable || false;
            items2[j].configurable = true;
            if ("value" in items2[j]) {
              items2[j].writable = true;
            }
            Object.defineProperty(result, items2[j].key, items2[j]);
          }
        }
        return result;
      }
      return defineClassMembers;
    }
    let classMembersInstaller = createClassMembersInstaller();
    function createExtensionRegistryClass() {
      let extensionRegistry;
      function ExtensionRegistry() {
        "use strict";
        if (this instanceof extensionRegistry) {
          this.extensions = Object.create(null);
          return void 0;
        } else {
          throw new TypeError("Cannot call a class as a function");
        }
      }
      extensionRegistry = ExtensionRegistry;
      function registerExtension(value, value2, value3, value4) {
        "use strict";
        this.extensions[value] = {
          "detector": value2,
          "serialiser": value3,
          deserialiser: value4
        };
      }
      classMembersInstaller(extensionRegistry, [{
        "key": "extend",
        value: registerExtension
      }]);
      return extensionRegistry;
    }
    j = createExtensionRegistryClass();
    let record7 = {};
    prototype = { detector: $THREAD_2_12162 };
    function $THREAD_4_47627() {
      throw new Error("serialisation is hard-coded into the encoder");
    }
    prototype.serialiser = $THREAD_4_47627;
    prototype.deserialiser = $THREAD_2_10761;
    let prototype2 = prototype;
    function $THREAD_5_81386() {
      function $THREAD_3_100741(result, items, items2) {
        if (items) {
          let i = 0;
          for (; i < items.length; i++) {
            items[i].enumerable = items[i].enumerable || false;
            items[i].configurable = true;
            if ("value" in items[i]) {
              items[i].writable = true;
            }
            Object.defineProperty(result.prototype, items[i].key, items[i]);
          }
        }
        if (items2) {
          for (let j = 0; j < items2.length; j++) {
            items2[j].enumerable = items2[j].enumerable || false;
            items2[j].configurable = true;
            if ("value" in items2[j]) {
              items2[j].writable = true;
            }
            Object.defineProperty(result, items2[j].key, items2[j]);
          }
        }
        return result;
      }
      return $THREAD_3_100741;
    }
    let result17 = $THREAD_5_81386();
    let floatBytesWriter;
    if (typeof Float32Array === "function" && typeof Float64Array === "function" && typeof Uint8Array === "function") {
      function writeFloatBytesWithTypedArrays(value, items) {
        let fLOAT32 = encodingTags.FLOAT32;
        let floats = new Float32Array(1);
        floats[0] = value;
        if (floats[0] !== value) {
          fLOAT32 = encodingTags.DOUBLE64;
          floats = new Float64Array(1);
          floats[0] = value;
        }
        floats = new Uint8Array(floats.buffer);
        items.push(fLOAT32);
        fLOAT32 = floats.length;
        for (;;) {
          --fLOAT32;
          if (!(fLOAT32 >= 0)) break;
          items.push(floats[fLOAT32]);
        }
      }
      floatBytesWriter = writeFloatBytesWithTypedArrays;
    } else {
      function writeFloatBytesFallback(value, items) {
        const flag = value < 0 || value === 0 && 1 / value < 0;
        let absolute = Math.abs(value);
        let floored, power;
        if (absolute === 0) {
          floored = 0;
          power = 1023;
        } else if (absolute >= Math.pow(2, -1022)) {
          let minimum = Math.min(Math.floor(Math.log(absolute) / Math.LN2), 1023);
          let ratio = absolute / Math.pow(2, minimum);
          if (ratio < 1) {
            --minimum;
            ratio = ratio * 2;
          }
          if (ratio >= 2) {
            ++minimum;
            ratio = ratio / 2;
          }
          power = Math.pow(2, 52);
          ratio = ratio * power;
          let floored2 = Math.floor(ratio);
          let delta = (ratio - floored2 < .5 ? floored2 : ratio - floored2 > .5 ? floored2 + 1 : floored2 % 2 ? floored2 + 1 : floored2) - power;
          minimum += 1023;
          power = delta / power >= 1 ? (delta = 0, minimum + 1) : minimum;
          power = power > 2046 ? (floored = 0, 2047) : (floored = delta, power);
        } else {
          let ratio = absolute / Math.pow(2, -1074);
          floored = Math.floor(ratio);
          ratio = ratio - floored;
          floored = ratio < .5 ? floored : ratio > .5 ? floored + 1 : floored % 2 ? floored + 1 : floored;
          power = 0;
        }
        let i = encodingTags.DOUBLE64;
        let count;
        if ((floored & 536870911) === 0 && Math.abs(power - 1023) < 256) {
          i = encodingTags.FLOAT32;
          floored = floored / 536870911;
          if (absolute < Math.pow(2, -126)) {
            absolute = absolute / Math.pow(2, -149);
            floored = Math.floor(absolute);
            absolute = absolute - floored;
            floored = absolute < .5 ? floored : absolute > .5 ? floored + 1 : floored % 2 ? floored + 1 : floored;
            power = 0;
          } else {
            power = power + 1023 - 127;
          }
          absolute = 23;
          count = 8;
        } else {
          absolute = 52;
          count = 11;
        }
        let items2 = [];
        while (absolute - 1 >= 0) {
          items2.unshift(floored & 1);
          --absolute;
          floored = Math.floor(floored / 2);
        }
        while (count > 0) {
          items2.unshift(power & 1);
          --count;
          power = Math.floor(power / 2);
        }
        items2.unshift(flag ? 1 : 0);
        items.push(i);
        for (i = 0; i < items2.length; i = i + 8) {
          items.push(items2[i] << 7 | items2[i + 1] << 6 | items2[i + 2] << 5 | items2[i + 3] << 4 | items2[i + 4] << 3 | items2[i + 5] << 2 | items2[i + 6] << 1 | items2[i + 7]);
        }
      }
      floatBytesWriter = writeFloatBytesFallback;
    }

//   Encoder helpers.
    function $THREAD_5_43056(value) {
      return value === result13;
    }
    function $THREAD_5_101057(value) {
      let result;
      function $THREAD_5_133660(value2, value3) {
        "use strict";
        let result4 = result;
        if (!(this instanceof result4)) {
          throw new TypeError("Cannot call a class as a function");
        }
        if (value2 != null && value3 == null || value2 == null && value3 != null) {
          throw new TypeError("depthBound must be specified if and only if depthBoundExtensionPoint is");
        }
        result4 = (result.__proto__ || Object.getPrototypeOf(result)).call(this);
        if (this) {
          result4 = result4 && (typeof result4 === "object" || typeof result4 === "function") ? result4 : this;
          result4.keysets = [];
          result4.stringHist = {};
          result4.stringPlaceholders = true;
          result4.remainingDepth = value2 == null ? null : value2;
          result4.depthBoundExtensionPoint = value3 == null ? null : value3;
          return result4;
        } else {
          throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
        }
      }
      result = $THREAD_5_133660;
      if (typeof value !== "function" && value !== null) {
        throw new TypeError("Super expression must either be null or a function, not " + typeof value);
      } else {
        let object = Object;
        let create = object.create;
        let value2 = value && value.prototype;
        let record = {
          value: $THREAD_5_133660,
          enumerable: false,
          writable: true,
          configurable: true
        };
        let record2 = { constructor: record };
        $THREAD_5_133660.prototype = create.call(object, value2, record2);
        if (value) {
          if (Object.setPrototypeOf) {
            Object.setPrototypeOf($THREAD_5_133660, value);
          } else {
            $THREAD_5_133660.__proto__ = value;
          }
        }
        value2 = result17;
        object = result;
        function $THREAD_5_71448() {
          "use strict";
          let flag = arguments.length <= 1 || arguments[1] === undefined;
          let valueEncoded = arguments[0];
          flag = flag ? {} : arguments[1];
          let items = [];
          if (flag.keysetsToOmit != null) {
            Array.prototype.push.apply(this.keysets, flag.keysetsToOmit);
          }
          valueEncoded = this.encodeValue(valueEncoded, []);
          if (flag.keysetsToOmit != null) {
            this.keysets = this.keysets.slice(flag.keysetsToOmit.length);
          }
          flag = this.encodeValue(this.keysets, []);
          let stringTableSelector2 = stringTableSelector(this.stringHist);
          this.stringPlaceholders = false;
          (stringTableSelector2.length > 0 || this.keysets.length > 0 ? (items.push(encodingTags.STRLUT), items.push(stringTableSelector2.length), this.pushArrayElements(stringTableSelector2, items), flag.concat(valueEncoded)) : valueEncoded).forEach((value3) => {
            if (typeof value3 === "string") {
              let items2 = items;
              let result4 = stringTableSelector2 ? stringTableSelector2.indexOf(value3) : -1;
              if (result4 !== -1) {
                items2.push(encodingTags.STRREF, result4);
              } else {
                result4 = unescape(encodeURIComponent(value3));
                let i = 0;
                let j = false;
                let items3 = [];
                for (; i < result4.length; i++) {
                  items3.push(result4.charCodeAt(i));
                  j = items3[i] === 0 || j;
                }
                i = items3.length;
                if (i < 32) {
                  items2.push(encodingTags.STR5_BASE | i);
                } else if (!j) {
                  items2.push(encodingTags.CSTRING);
                  items3.push(0);
                } else if (i <= 255) {
                  items2.push(encodingTags.STR8, i);
                } else {
                  items2.push(encodingTags.STR_);
                  lengthByteWriter(i, items2);
                }
                if (items3.length > 65535) {
                  for (j = 0; j < items3.length; j = j + 65535) {
                    Array.prototype.push.apply(items2, items3.slice(j, j + 65535));
                  }
                } else {
                  Array.prototype.push.apply(items2, items3);
                }
              }
            } else {
              items.push(value3);
            }
          });
          return items;
        }
        create = [{
          "key": "encode",
          value: $THREAD_5_71448
        }];
        let record3 = {
          key: "findKeysetIndex",
          value: void 0
        };
        function $THREAD_2_43338(value3) {
          "use strict";
          let value32 = value3;
          let keysets = this.keysets;
          function $THREAD_8_46933(value4) {
            return arrayContentsEqual(value4, value32);
          }
          let count = 0;
          let value5;
          for (;;) {
            value5 = void 0;
            if (count >= keysets.length) {
              value5 = -1;
              break;
            }
            if (!$THREAD_8_46933(keysets[count])) {
              ++count;
              continue;
            }
            value5 = count;
            break;
          }
          if (value5 < 0) {
            return this.keysets.push(value32) - 1;
          } else {
            return value5;
          }
        }
        record3.value = $THREAD_2_43338;
        create[1] = record3;
        let record4 = {
          key: "pushArrayElements",
          value: void 0
        };
        function $THREAD_8_78678(value3, value4) {
          "use strict";
          let value42 = value4;
          let value5 = this;
          Array.prototype.forEach.call(value3, (value6) => {
            value5.encodeValue(value6, value42);
          });
        }
        record4.value = $THREAD_8_78678;
        create[2] = record4;
        let record6 = {
          key: "encodeValue",
          value: void 0
        };
        function $THREAD_3_8295(value3, value4) {
          "use strict";
          let value42 = value4;
          let value32 = value3;
          let value5 = this;
          let time = Object.keys(this.extensions);
          function $THREAD_6_53818(value6) {
            return value5.extensions[value6].detector(value32);
          }
          let k = 0;
          let i;
          for (;;) {
            i = void 0;
            if (!(k < time.length)) {
              i = null;
              break;
            }
            i = time[k];
            if (!$THREAD_6_53818(i)) {
              ++k;
              continue;
            }
            break;
          }
          if (i != null) {
            value42.push(encodingTags.EXTENSION);
            lengthByteWriter(Number(i), value42);
            this.encodeValue(this.extensions[Number(i)].serialiser(value32), value42);
          } else if (value32 === false) {
            value42.push(encodingTags.FALSE);
          } else if (value32 === true) {
            value42.push(encodingTags.TRUE);
          } else if (value32 === null) {
            value42.push(encodingTags.NULL);
          } else if (typeof value32 === "undefined") {
            value42.push(encodingTags.UNDEFINED);
          } else if (typeof value32 === "number") {
            if (isFinite(value32)) {
              i = Math.abs(value32);
              if (Math.floor(i) === i && i < 0xfffffffffffff800 && (i !== 0 || 1 / value32 > 0)) {
                k = value32;
                time = value42;
                i = void 0;
                if (k === 0) {
                  i = 1 / k === -Infinity;
                } else {
                  i = k === 0;
                }
                if (i) {
                  time.push(encodingTags.NINT4_BASE);
                } else if (k >= 0) {
                  lengthByteWriter(k, time);
                } else {
                  k = -k;
                  if (k <= 15) {
                    time.push(encodingTags.NINT4_BASE | k);
                  } else if (k <= 255) {
                    time.push(encodingTags.NINT8, k);
                  } else if (k <= 65535) {
                    time.push(encodingTags.NINT16, k >> 8, k & 255);
                  } else if (k <= 4294967295) {
                    time.push(encodingTags.NINT32);
                    time.push(k >>> 24, k >> 16 & 255, k >> 8 & 255, k & 255);
                  } else {
                    time.push(encodingTags.NINT64);
                    i = k / 4294967296 & 4294967295;
                    time.push(i >>> 24, i >> 16 & 255, i >> 8 & 255, i & 255);
                    i = time.push;
                    i.call(time, (k & 4294967295) >>> 24, (k & 4294967295) >> 16 & 255, (k & 4294967295) >> 8 & 255, k & 4294967295 & 255);
                  }
                }
              } else {
                floatBytesWriter(value32, value42);
              }
            } else if (value32 === 1 / 0) {
              value42.push(encodingTags.FLOAT32, 127, 128, 0, 0);
            } else if (value32 === -1 / 0) {
              value42.push(encodingTags.FLOAT32, 255, 128, 0, 0);
            } else if (value32 !== value32) {
              value42.push(encodingTags.FLOAT32, 127, 192, 0, 0);
            }
          } else if (typeof value32 === "string") {
            if (this.stringPlaceholders) {
              this.stringHist[value32] = (this.stringHist[value32] || 0) + 1;
              value42.push(value32);
            } else {
              time = value42;
              k = unescape(encodeURIComponent(value32));
              i = 0;
              let j = false;
              let items = [];
              for (; i < k.length; i++) {
                items.push(k.charCodeAt(i));
                j = items[i] === 0 || j;
              }
              i = items.length;
              if (i < 32) {
                time.push(encodingTags.STR5_BASE | i);
              } else if (!j) {
                time.push(encodingTags.CSTRING);
                items.push(0);
              } else if (i <= 255) {
                time.push(encodingTags.STR8, i);
              } else {
                time.push(encodingTags.STR_);
                lengthByteWriter(i, time);
              }
              if (items.length > 65535) {
                for (j = 0; j < items.length; j = j + 65535) {
                  Array.prototype.push.apply(time, items.slice(j, j + 65535));
                }
              } else {
                Array.prototype.push.apply(time, items);
              }
            }
          } else if (Object.prototype.toString.call(value32) === "[object Date]") {
            k = value42;
            time = Date.prototype.getTime.call(value32);
            i = time / 4294967296;
            i = time < 0 ? i - 1 : i;
            k.push(encodingTags.TIMESTAMP, i >>> 8 & 255, i & 255);
            i = k.push;
            i.call(k, time >>> 0 >>> 24, time >>> 0 >> 16 & 255, time >>> 0 >> 8 & 255, time >>> 0 & 255);
          } else if (typeof ArrayBuffer !== "undefined" && value32 instanceof ArrayBuffer) {
            value42.push(encodingTags.BINARY_);
            this.encodeValue(value32.byteLength, value42);
            this.pushArrayElements(new Uint8Array(value32), value42);
          } else {
            if (this.remainingDepth !== null) {
              if (this.remainingDepth <= 0) {
                value42.push(encodingTags.EXTENSION);
                if (this.depthBoundExtensionPoint == null) {
                  throw new Error("if remainingDepth != null, depthBoundExtensionPoint must not be either");
                } else {
                  lengthByteWriter(this.depthBoundExtensionPoint, value42);
                  value42.push(encodingTags.NULL);
                  return value42;
                }
              } else {
                this.remainingDepth = Number(this.remainingDepth) - 1;
              }
            }
            if (Array.isArray(value32)) {
              time = value32.length;
              if (value32.every((value6) => typeof value6 === "boolean") && time > 0) {
                if (time <= 15) {
                  value42.push(encodingTags.BARRAY4_BASE | time);
                } else if (time <= 255) {
                  value42.push(encodingTags.BARRAY8, time);
                } else {
                  value42.push(encodingTags.BARRAY_);
                  lengthByteWriter(time, value42);
                }
                for (k = 0; k < time; k = k + 8) {
                  value42.push(value32[k] << 7 | value32[k + 1] << 6 | value32[k + 2] << 5 | value32[k + 3] << 4 | value32[k + 4] << 3 | value32[k + 5] << 2 | value32[k + 6] << 1 | value32[k + 7]);
                }
              } else {
                if (time <= 31) {
                  value42.push(encodingTags.ARRAY5_BASE | time);
                } else if (time <= 255) {
                  value42.push(encodingTags.ARRAY8, time);
                } else {
                  value42.push(encodingTags.ARRAY_);
                  lengthByteWriter(time, value42);
                }
                this.pushArrayElements(value32, value42);
              }
            } else {
              time = Object.keys(value32).sort();
              i = time.length;
              k = this.findKeysetIndex(time);
              if (time.every((value6) => typeof value32[value6] === "boolean")) {
                value42.push(encodingTags.BMAP_);
                lengthByteWriter(k, value42);
                k = 0;
                let items = [
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0,
                  0
                ];
                while (k < i) {
                  for (let i_2 = 0; i_2 < 8; i_2++) {
                    items[i_2] = k + i_2 < i && value32[time[k + i_2]];
                  }
                  k += 8;
                  value42.push(items[0] << 7 | items[1] << 6 | items[2] << 5 | items[3] << 4 | items[4] << 3 | items[5] << 2 | items[6] << 1 | items[7]);
                }
              } else {
                value42.push(encodingTags.MAP_);
                lengthByteWriter(k, value42);
                time.forEach((value6) => value5.encodeValue(value32[value6], value42));
              }
            }
            if (this.remainingDepth !== null) {
              this.remainingDepth = Number(this.remainingDepth) + 1;
            }
          }
          return value42;
        }
        record6.value = $THREAD_3_8295;
        create[3] = record6;
        function $THREAD_2_14639() {
          let flag = arguments.length <= 1 || arguments[1] === undefined;
          let value3 = flag ? {} : arguments[1];
          let value4 = null;
          if (value3.depthBound != null) {
            flag = value3.depthBound;
            if (flag < 0 || Math.floor(flag) !== flag) {
              throw new RangeError("depthBound, if specified, must be a non-negative integer");
            }
            if (value3.extensions != null) {
              Object.keys(value3.extensions).forEach((value5) => {
                let number4 = Number(value5);
                if (value3.extensions[number4] === prototype2) {
                  value4 = number4;
                }
              });
            }
            if (value4 == null) {
              throw new Error("if depthBound is used, its corresponding extension must be provided");
            }
          } else {
            flag = value3.extensions != null;
            if (flag) {
              flag = Object.keys(value3.extensions).some((value5) => value3.extensions[value5] === prototype2);
            }
            if (flag) {
              throw new Error("if the depthBound extension is used, a depth bound must be specified");
            }
          }
          let instance = new result(value3.depthBound, value4);
          if (value3.extensions != null) {
            Object.keys(value3.extensions).forEach((value5) => {
              instance.extend(value5, value3.extensions[value5].detector, value3.extensions[value5].serialiser, value3.extensions[value5].deserialiser);
            });
          }
          return instance.encode(arguments[0], value3);
        }
        value2(object, create, [{
          "key": "encode",
          value: $THREAD_2_14639
        }]);
        return result;
      }
    }
    let encode = $THREAD_5_101057(j).encode;
    prototype = { detector: $THREAD_5_43056 };
    function $THREAD_5_121298() {}
    prototype.serialiser = $THREAD_5_121298;
    prototype.deserialiser = $THREAD_5_65642;
    let prototype3 = prototype;
function signalOptions(keysetsToOmit=[],depthBound=100) {
 return {keysetsToOmit,depthBound,extensions:{0:prototype3,3735928559:prototype2}};
}
module.exports = {packValue:encode, signalOptions, missingSentinel:result13};
