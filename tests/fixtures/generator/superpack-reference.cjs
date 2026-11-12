//                                                               Exact serializer reference implementation.
'use strict';
let result8 = Object.create(null); //  Null-prototype identity sentinel.
let encodingTags, handler2, j, value53, encode2;

//  Array equality.
    function arraysEqualByIndex(items, items2) {
      let items22 = items2;
      if (items.length === items2.length) {
        return items.every((value, value3) => items22[value3] === value);
      } else {
        return false;
      }
    }

//    Data helpers.
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
    let lengthByteWriter = writeUnsignedIntBytes;
    function selectStringTable(value) {
      let value3 = value;
      return Object.keys(value3).filter((items) => value3[items] >= 2 && items.length * value3[items] >= 8).map((items) => {
        return [items, (items.length + 1) * value3[items] - (items.length + 1 + value3[items] * 2)];
      }).sort((items, items2) => items2[1] - items[1]).slice(0, 255).map((items) => items[0]);
    }
    let stringTableSelector = selectStringTable;
    let arrayContentsEqual = arraysEqualByIndex;

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
      function defineClassMembers(result2, items, items2) {
        if (items) {
          let i = 0;
          for (; i < items.length; i++) {
            items[i].enumerable = items[i].enumerable || false;
            items[i].configurable = true;
            if ("value" in items[i]) {
              items[i].writable = true;
            }
            Object.defineProperty(result2.prototype, items[i].key, items[i]);
          }
        }
        if (items2) {
          for (let j = 0; j < items2.length; j++) {
            items2[j].enumerable = items2[j].enumerable || false;
            items2[j].configurable = true;
            if ("value" in items2[j]) {
              items2[j].writable = true;
            }
            Object.defineProperty(result2, items2[j].key, items2[j]);
          }
        }
        return result2;
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
      function registerExtension(value, value3, value4, value5) {
        "use strict";
        this.extensions[value] = {
          "detector": value3,
          "serialiser": value4,
          deserialiser: value5
        };
      }
      classMembersInstaller(extensionRegistry, [{
        "key": "extend",
        value: registerExtension
      }]);
      return extensionRegistry;
    }
    handler2 = createExtensionRegistryClass();
    let record9 = {};
    function createAgentModuleDetector() {
      return false;
    }
    j = { detector: createAgentModuleDetector };
    function jSerialiser() {
      throw new Error("serialisation is hard-coded into the encoder");
    }
    j.serialiser = jSerialiser;
    function jDeserialiser() {
      return record9;
    }
    j.deserialiser = jDeserialiser;
    let j3 = j;
    function installClassMembers() {
      function installClassMembers2(result2, items, items2) {
        if (items) {
          let i = 0;
          for (; i < items.length; i++) {
            items[i].enumerable = items[i].enumerable || false;
            items[i].configurable = true;
            if ("value" in items[i]) {
              items[i].writable = true;
            }
            Object.defineProperty(result2.prototype, items[i].key, items[i]);
          }
        }
        if (items2) {
          for (let j = 0; j < items2.length; j++) {
            items2[j].enumerable = items2[j].enumerable || false;
            items2[j].configurable = true;
            if ("value" in items2[j]) {
              items2[j].writable = true;
            }
            Object.defineProperty(result2, items2[j].key, items2[j]);
          }
        }
        return result2;
      }
      return installClassMembers2;
    }
    let installClassMembers3 = installClassMembers();
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
        let floored, ratio;
        if (absolute === 0) {
          floored = 1023;
          ratio = 0;
        } else if (absolute >= Math.pow(2, -1022)) {
          let minimum = Math.min(Math.floor(Math.log(absolute) / Math.LN2), 1023);
          let ratio2 = absolute / Math.pow(2, minimum);
          let sum;
          ratio2 = ratio2 < 1 ? (sum = minimum - 1, ratio2 * 2) : (sum = minimum, ratio2);
          ratio2 = ratio2 >= 2 ? (sum = sum + 1, ratio2 / 2) : ratio2;
          ratio = Math.pow(2, 52);
          ratio2 = ratio2 * ratio;
          let floored2 = Math.floor(ratio2);
          ratio2 = (ratio2 - floored2 < .5 ? floored2 : ratio2 - floored2 > .5 ? floored2 + 1 : floored2 % 2 ? floored2 + 1 : floored2) - ratio;
          sum += 1023;
          ratio = ratio2 / ratio >= 1 ? (sum = sum + 1, 0) : ratio2;
          ratio = sum > 2046 ? (floored = 2047, 0) : (floored = sum, ratio);
        } else {
          let ratio2 = absolute / Math.pow(2, -1074);
          ratio = Math.floor(ratio2);
          ratio2 = ratio2 - ratio;
          floored = 0;
          ratio = ratio2 < .5 ? ratio : ratio2 > .5 ? ratio + 1 : ratio % 2 ? ratio + 1 : ratio;
        }
        let i = encodingTags.DOUBLE64;
        let count;
        if ((ratio & 536870911) === 0 && Math.abs(floored - 1023) < 256) {
          i = encodingTags.FLOAT32;
          ratio = ratio / 536870911;
          if (absolute < Math.pow(2, -126)) {
            ratio = absolute / Math.pow(2, -149);
            let floored2 = Math.floor(ratio);
            floored = 0;
            ratio = ratio - floored2 < .5 ? floored2 : ratio - floored2 > .5 ? floored2 + 1 : floored2 % 2 ? floored2 + 1 : floored2;
          } else {
            floored = floored + 1023 - 127;
          }
          absolute = 23;
          count = 8;
        } else {
          absolute = 52;
          count = 11;
        }
        let items2 = [];
        while (absolute - 1 >= 0) {
          items2.unshift(ratio & 1);
          --absolute;
          ratio = Math.floor(ratio / 2);
        }
        while (count > 0) {
          items2.unshift(floored & 1);
          --count;
          floored = Math.floor(floored / 2);
        }
        items2.unshift(flag ? 1 : 0);
        items.push(i);
        for (i = 0; i < items2.length; i = i + 8) {
          items.push(items2[i] << 7 | items2[i + 1] << 6 | items2[i + 2] << 5 | items2[i + 3] << 4 | items2[i + 4] << 3 | items2[i + 5] << 2 | items2[i + 6] << 1 | items2[i + 7]);
        }
      }
      floatBytesWriter = writeFloatBytesFallback;
    }

//     Value encoder.
    function createSignalEncoder(value) {
      let createSignalEncoderValue4;
      function createSignalEncoderValue(value3, value4) {
        "use strict";
        let createSignalEncoderValue42 = createSignalEncoderValue4;
        if (!(this instanceof createSignalEncoderValue42)) {
          throw new TypeError("Cannot call a class as a function");
        }
        if (value3 != null && value4 == null || value3 == null && value4 != null) {
          throw new TypeError("depthBound must be specified if and only if depthBoundExtensionPoint is");
        }
        createSignalEncoderValue42 = (createSignalEncoderValue4.__proto__ || Object.getPrototypeOf(createSignalEncoderValue4)).call(this);
        if (this) {
          createSignalEncoderValue42 = createSignalEncoderValue42 && (typeof createSignalEncoderValue42 === "object" || typeof createSignalEncoderValue42 === "function") ? createSignalEncoderValue42 : this;
          createSignalEncoderValue42.keysets = [];
          createSignalEncoderValue42.stringHist = {};
          createSignalEncoderValue42.stringPlaceholders = true;
          createSignalEncoderValue42.remainingDepth = value3 == null ? null : value3;
          createSignalEncoderValue42.depthBoundExtensionPoint = value4 == null ? null : value4;
          return createSignalEncoderValue42;
        } else {
          throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
        }
      }
      createSignalEncoderValue4 = createSignalEncoderValue;
      if (typeof value !== "function" && value !== null) {
        throw new TypeError("Super expression must either be null or a function, not " + typeof value);
      } else {
        let object = Object;
        let create = object.create;
        let value3 = value && value.prototype;
        let record = {
          value: createSignalEncoderValue,
          enumerable: false,
          writable: true,
          configurable: true
        };
        let record2 = { constructor: record };
        createSignalEncoderValue.prototype = create.call(object, value3, record2);
        if (value) {
          if (Object.setPrototypeOf) {
            Object.setPrototypeOf(createSignalEncoderValue, value);
          } else {
            createSignalEncoderValue.__proto__ = value;
          }
        }
        value3 = installClassMembers3;
        object = createSignalEncoderValue4;
        function createSignalEncoderValue2() {
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
          (stringTableSelector2.length > 0 || this.keysets.length > 0 ? (items.push(encodingTags.STRLUT), items.push(stringTableSelector2.length), this.pushArrayElements(stringTableSelector2, items), flag.concat(valueEncoded)) : valueEncoded).forEach((value4) => {
            if (typeof value4 === "string") {
              let items2 = items;
              let result2 = stringTableSelector2 ? stringTableSelector2.indexOf(value4) : -1;
              if (result2 !== -1) {
                items2.push(encodingTags.STRREF, result2);
              } else {
                result2 = unescape(encodeURIComponent(value4));
                let i = 0;
                let j = false;
                let items3 = [];
                for (; i < result2.length; i++) {
                  items3.push(result2.charCodeAt(i));
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
              items.push(value4);
            }
          });
          return items;
        }
        create = [{
          "key": "encode",
          value: createSignalEncoderValue2
        }];
        let record3 = {
          key: "findKeysetIndex",
          value: void 0
        };
        function owner18Value(value4) {
          "use strict";
          let value42 = value4;
          function owner18ValueFunction1(value5) {
            return arrayContentsEqual(value5, value42);
          }
          let keysets = this.keysets;
          let count = 0;
          let value6;
          for (;;) {
            value6 = void 0;
            if (count >= keysets.length) {
              value6 = -1;
              break;
            }
            if (!owner18ValueFunction1(keysets[count])) {
              ++count;
              continue;
            }
            value6 = count;
            break;
          }
          if (value6 < 0) {
            return this.keysets.push(value42) - 1;
          } else {
            return value6;
          }
        }
        record3.value = owner18Value;
        create[1] = record3;
        let record4 = {
          key: "pushArrayElements",
          value: void 0
        };
        function owner19Value(value4, value5) {
          "use strict";
          let value52 = value5;
          let value6 = this;
          Array.prototype.forEach.call(value4, (value7) => {
            value6.encodeValue(value7, value52);
          });
        }
        record4.value = owner19Value;
        create[2] = record4;
        let record7 = {
          key: "encodeValue",
          value: void 0
        };
        function owner20Value(value4, value5) {
          "use strict";
          let value52 = value5;
          let value42 = value4;
          let value6 = this;
          let time = Object.keys(this.extensions);
          function detectorForExtension(value7) {
            return value6.extensions[value7].detector(value42);
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
            if (!detectorForExtension(i)) {
              ++k;
              continue;
            }
            break;
          }
          if (i != null) {
            value52.push(encodingTags.EXTENSION);
            lengthByteWriter(Number(i), value52);
            this.encodeValue(this.extensions[Number(i)].serialiser(value42), value52);
          } else if (value42 === false) {
            value52.push(encodingTags.FALSE);
          } else if (value42 === true) {
            value52.push(encodingTags.TRUE);
          } else if (value42 === null) {
            value52.push(encodingTags.NULL);
          } else if (typeof value42 === "undefined") {
            value52.push(encodingTags.UNDEFINED);
          } else if (typeof value42 === "number") {
            if (isFinite(value42)) {
              i = Math.abs(value42);
              if (Math.floor(i) === i && i < 0xfffffffffffff800 && (i !== 0 || 1 / value42 > 0)) {
                k = value42;
                time = value52;
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
                floatBytesWriter(value42, value52);
              }
            } else if (value42 === 1 / 0) {
              value52.push(encodingTags.FLOAT32, 127, 128, 0, 0);
            } else if (value42 === -1 / 0) {
              value52.push(encodingTags.FLOAT32, 255, 128, 0, 0);
            } else if (value42 !== value42) {
              value52.push(encodingTags.FLOAT32, 127, 192, 0, 0);
            }
          } else if (typeof value42 === "string") {
            if (this.stringPlaceholders) {
              this.stringHist[value42] = (this.stringHist[value42] || 0) + 1;
              value52.push(value42);
            } else {
              time = value52;
              k = unescape(encodeURIComponent(value42));
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
          } else if (Object.prototype.toString.call(value42) === "[object Date]") {
            k = value52;
            time = Date.prototype.getTime.call(value42);
            i = time / 4294967296;
            i = time < 0 ? i - 1 : i;
            k.push(encodingTags.TIMESTAMP, i >>> 8 & 255, i & 255);
            i = k.push;
            i.call(k, time >>> 0 >>> 24, time >>> 0 >> 16 & 255, time >>> 0 >> 8 & 255, time >>> 0 & 255);
          } else if (typeof ArrayBuffer !== "undefined" && value42 instanceof ArrayBuffer) {
            value52.push(encodingTags.BINARY_);
            this.encodeValue(value42.byteLength, value52);
            this.pushArrayElements(new Uint8Array(value42), value52);
          } else {
            if (this.remainingDepth !== null) {
              if (this.remainingDepth <= 0) {
                value52.push(encodingTags.EXTENSION);
                if (this.depthBoundExtensionPoint == null) {
                  throw new Error("if remainingDepth != null, depthBoundExtensionPoint must not be either");
                } else {
                  lengthByteWriter(this.depthBoundExtensionPoint, value52);
                  value52.push(encodingTags.NULL);
                  return value52;
                }
              } else {
                this.remainingDepth = Number(this.remainingDepth) - 1;
              }
            }
            if (Array.isArray(value42)) {
              time = value42.length;
              if (value42.every((value7) => typeof value7 === "boolean") && time > 0) {
                if (time <= 15) {
                  value52.push(encodingTags.BARRAY4_BASE | time);
                } else if (time <= 255) {
                  value52.push(encodingTags.BARRAY8, time);
                } else {
                  value52.push(encodingTags.BARRAY_);
                  lengthByteWriter(time, value52);
                }
                for (k = 0; k < time; k = k + 8) {
                  value52.push(value42[k] << 7 | value42[k + 1] << 6 | value42[k + 2] << 5 | value42[k + 3] << 4 | value42[k + 4] << 3 | value42[k + 5] << 2 | value42[k + 6] << 1 | value42[k + 7]);
                }
              } else {
                if (time <= 31) {
                  value52.push(encodingTags.ARRAY5_BASE | time);
                } else if (time <= 255) {
                  value52.push(encodingTags.ARRAY8, time);
                } else {
                  value52.push(encodingTags.ARRAY_);
                  lengthByteWriter(time, value52);
                }
                this.pushArrayElements(value42, value52);
              }
            } else {
              time = Object.keys(value42).sort();
              i = time.length;
              k = this.findKeysetIndex(time);
              if (time.every((value7) => typeof value42[value7] === "boolean")) {
                value52.push(encodingTags.BMAP_);
                lengthByteWriter(k, value52);
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
                    items[i_2] = k + i_2 < i ? value42[time[k + i_2]] : k + i_2 < i;
                  }
                  value52.push(items[0] << 7 | items[1] << 6 | items[2] << 5 | items[3] << 4 | items[4] << 3 | items[5] << 2 | items[6] << 1 | items[7]);
                  k += 8;
                }
              } else {
                value52.push(encodingTags.MAP_);
                lengthByteWriter(k, value52);
                time.forEach((value7) => value6.encodeValue(value42[value7], value52));
              }
            }
            if (this.remainingDepth !== null) {
              this.remainingDepth = Number(this.remainingDepth) + 1;
            }
          }
          return value52;
        }
        record7.value = owner20Value;
        create[3] = record7;
        function createSignalEncoderValue3() {
          let flag = arguments.length <= 1 || arguments[1] === undefined;
          let value4 = flag ? {} : arguments[1];
          let value5 = null;
          if (value4.depthBound != null) {
            flag = value4.depthBound;
            if (flag < 0 || Math.floor(flag) !== flag) {
              throw new RangeError("depthBound, if specified, must be a non-negative integer");
            }
            if (value4.extensions != null) {
              Object.keys(value4.extensions).forEach((value6) => {
                let number4 = Number(value6);
                if (value4.extensions[number4] === j3) {
                  value5 = number4;
                }
              });
            }
            if (value5 == null) {
              throw new Error("if depthBound is used, its corresponding extension must be provided");
            }
          } else {
            flag = value4.extensions != null;
            if (flag) {
              flag = Object.keys(value4.extensions).some((value6) => value4.extensions[value6] === j3);
            }
            if (flag) {
              throw new Error("if the depthBound extension is used, a depth bound must be specified");
            }
          }
          let instance = new createSignalEncoderValue4(value4.depthBound, value5);
          if (value4.extensions != null) {
            Object.keys(value4.extensions).forEach((value6) => {
              instance.extend(value6, value4.extensions[value6].detector, value4.extensions[value6].serialiser, value4.extensions[value6].deserialiser);
            });
          }
          return instance.encode(arguments[0], value4);
        }
        value3(object, create, [{
          "key": "encode",
          value: createSignalEncoderValue3
        }]);
        return createSignalEncoderValue4;
      }
    }
    encode2 = createSignalEncoder(handler2).encode;
    function createAgentModuleDetector2(value) {
      return value === result8;
    }
    let record10 = { detector: createAgentModuleDetector2 };
    function owner21Serialiser() {}
    record10.serialiser = owner21Serialiser;
    function owner21Deserialiser() {
      return result8;
    }
    record10.deserialiser = owner21Deserialiser;
    value53 = record10;
function packValue(value, options) { return encode2(value, options); }
function signalOptions(keysetsToOmit = [], depthBound = 100) {
 return {keysetsToOmit, depthBound, extensions: {0: value53, 3735928559: j3}};
}
module.exports = {packValue, signalOptions, missingSentinel:result8};
