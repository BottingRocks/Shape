//! Complete interned-string factory alternatives, retaining cache, cursor,
//! surrogate-pair, mask order, publication and captured producer identities.
pub(super) const FORMS: &[&str] = &[
r###"function factory(maskInput) {
    let slot0 = maskInput;
    let slot1 = Object.create(null);
    let slot2 = 0;
    function slot3(slot4, slot5, slot6) {
      let slot7;
      if (slot1[slot4] != null) {
        slot7 = slot6;
        slot7 = Number(slot6) + 1;
        slot5[slot7] = slot0() ^ 129;
        slot7 = numberWriter(slot0, slot1[slot4], slot5, slot7);
      } else {
        let slot8 = slot4.length;
        let slot9 = 0;
        let slot10 = slot6;
        while (slot9 < slot8) {
          let slot11 = characterCode(slot4[slot9], 0);
          if (slot11 === 0) {
            slot5[++slot10] = slot0() ^ 192;
            slot11 = slot0() ^ 128;
          } else if (slot11 < 128) {
            slot11 = slot11 ^ slot0();
          } else if (slot11 < 2048) {
            slot5[++slot10] = (slot11 >> 6 | 192) ^ slot0();
            slot11 = (slot11 & 63 | 128) ^ slot0();
          } else {
            let slot12 = slot11 >= 55296 && slot11 <= 57343 && slot11 < 56320 && slot9 + 1 < slot8 ? characterCode(slot4[slot9 + 1], 0) : null;
            if (slot12 == null || slot12 < 56320 || slot12 > 57343) {
              slot5[++slot10] = (slot11 >> 12 | 224) ^ slot0();
              slot5[++slot10] = (slot11 >> 6 & 63 | 128) ^ slot0();
              slot11 = (slot11 & 63 | 128) ^ slot0();
            } else {
              ++slot9;
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 18 | 240) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 12 & 63 | 128) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 6 & 63 | 128) ^ slot0();
              slot11 = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) & 63 | 128) ^ slot0();
            }
          }
          slot5[++slot10] = slot11;
          ++slot9;
        }
        slot7 = Number(slot10) + 1;
        slot5[slot7] = slot0();
        let slot13 = slot1;
        let slot14 = Number(slot2);
        slot2 = slot14 + 1;
        slot13[slot4] = slot14;
      }
      return slot7;
    }
    return slot3;
  }"###,
r###"function factory(maskInput) {
    let slot0 = maskInput;
    let slot1;
    slot1 = Object.create(null);
    let slot2 = 0;
    function slot3(slot4, slot5, slot6) {
      let slot7;
      if (slot1[slot4] != null) {
        slot7 = slot6;
        slot7 = Number(slot6) + 1;
        slot5[slot7] = slot0() ^ 129;
        slot7 = numberWriter(slot0, slot1[slot4], slot5, slot7);
      } else {
        let slot8 = slot4.length;
        let slot9 = 0;
        let slot10 = slot6;
        while (slot9 < slot8) {
          let slot11 = characterCode(slot4[slot9], 0);
          let slot12, slot13, slot14;
          if (slot11 === 0) {
            slot5[++slot10] = slot0() ^ 192;
            slot13 = slot0();
            slot14 = 128;
          } else {
            if (slot11 < 128) {
              slot13 = slot11;
            } else if (slot11 < 2048) {
              slot5[++slot10] = (slot11 >> 6 | 192) ^ slot0();
              slot13 = slot11 & 63 | 128;
            } else {
              const slot15 = slot11 >= 55296 && slot11 <= 57343 && slot11 < 56320 && slot9 + 1 < slot8 ? characterCode(slot4[slot9 + 1], 0) : null;
              if (slot15 == null || slot15 < 56320 || slot15 > 57343) {
                slot5[++slot10] = (slot11 >> 12 | 224) ^ slot0();
                slot5[++slot10] = (slot11 >> 6 & 63 | 128) ^ slot0();
                slot13 = slot11 & 63 | 128;
              } else {
                ++slot9;
                slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot15 - 56320) >> 18 | 240) ^ slot0();
                slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot15 - 56320) >> 12 & 63 | 128) ^ slot0();
                slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot15 - 56320) >> 6 & 63 | 128) ^ slot0();
                slot13 = 65536 + (slot11 - 55296 << 10) + (slot15 - 56320) & 63 | 128;
              }
            }
            slot14 = slot0();
          }
          slot12 = Number(slot10) + 1;
          slot5[slot12] = slot13 ^ slot14;
          ++slot9;
          slot10 = slot12;
        }
        slot7 = Number(slot10) + 1;
        slot5[slot7] = slot0();
        let slot16 = slot1;
        let slot17 = Number(slot2);
        slot2 = slot17 + 1;
        slot16[slot4] = slot17;
      }
      return slot7;
    }
    return slot3;
  }"###,
r###"function factory(maskInput) {
    let slot0 = maskInput;
    let slot1;
    slot1 = Object.create(null);
    let slot2 = 0;
    function slot3(slot4, slot5, slot6) {
      let slot7;
      if (slot1[slot4] != null) {
        slot7 = slot6;
        slot5[++slot7] = slot0() ^ 129;
        slot7 = numberWriter(slot0, slot1[slot4], slot5, slot7);
      } else {
        let slot8 = slot4.length;
        let slot9 = 0;
        let slot10 = slot6;
        while (slot9 < slot8) {
          let slot11 = characterCode(slot4[slot9], 0);
          if (slot11 === 0) {
            slot5[++slot10] = slot0() ^ 192;
            slot11 = slot0() ^ 128;
          } else if (slot11 < 128) {
            slot11 = slot11 ^ slot0();
          } else if (slot11 < 2048) {
            slot5[++slot10] = (slot11 >> 6 | 192) ^ slot0();
            slot11 = (slot11 & 63 | 128) ^ slot0();
          } else {
            const slot12 = slot11 >= 55296 && slot11 <= 57343 && slot11 < 56320 && slot9 + 1 < slot8 ? characterCode(slot4[slot9 + 1], 0) : null;
            if (slot12 == null || slot12 < 56320 || slot12 > 57343) {
              slot5[++slot10] = (slot11 >> 12 | 224) ^ slot0();
              slot5[++slot10] = (slot11 >> 6 & 63 | 128) ^ slot0();
              slot11 = (slot11 & 63 | 128) ^ slot0();
            } else {
              ++slot9;
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 18 | 240) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 12 & 63 | 128) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 6 & 63 | 128) ^ slot0();
              slot11 = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) & 63 | 128) ^ slot0();
            }
          }
          slot5[++slot10] = slot11;
          ++slot9;
        }
        slot7 = Number(slot10) + 1;
        slot5[slot7] = slot0();
        let slot13 = slot1;
        let slot14 = Number(slot2);
        slot2 = slot14 + 1;
        slot13[slot4] = slot14;
      }
      return slot7;
    }
    return slot3;
  }"###,
r###"function factory(maskInput) {
    let slot0 = maskInput;
    let slot1 = Object.create(null);
    let slot2 = 0;
    function slot3(slot4, slot5, slot6) {
      let slot7;
      if (slot1[slot4] != null) {
        let slot8 = Number(slot6) + 1;
        slot5[slot8] = slot0() ^ 129;
        slot7 = numberWriter(slot0, slot1[slot4], slot5, slot8);
      } else {
        let slot9 = slot4.length;
        let slot10 = 0;
        let slot11 = slot6;
        while (slot10 < slot9) {
          let slot12 = characterCode(slot4[slot10], 0);
          if (slot12 === 0) {
            slot5[++slot11] = slot0() ^ 192;
            slot12 = 128;
          } else if (!(slot12 < 128)) if (slot12 < 2048) {
            slot5[++slot11] = (slot12 >> 6 | 192) ^ slot0();
            slot12 = slot12 & 63 | 128;
          } else {
            const slot13 = slot12 >= 55296 && slot12 <= 57343 && slot12 < 56320 && slot10 + 1 < slot9 ? characterCode(slot4[slot10 + 1], 0) : null;
            if (slot13 == null || slot13 < 56320 || slot13 > 57343) {
              slot5[++slot11] = (slot12 >> 12 | 224) ^ slot0();
              slot5[++slot11] = (slot12 >> 6 & 63 | 128) ^ slot0();
              slot12 = slot12 & 63 | 128;
            } else {
              ++slot10;
              slot5[++slot11] = (65536 + (slot12 - 55296 << 10) + (slot13 - 56320) >> 18 | 240) ^ slot0();
              slot5[++slot11] = (65536 + (slot12 - 55296 << 10) + (slot13 - 56320) >> 12 & 63 | 128) ^ slot0();
              slot5[++slot11] = (65536 + (slot12 - 55296 << 10) + (slot13 - 56320) >> 6 & 63 | 128) ^ slot0();
              slot12 = 65536 + (slot12 - 55296 << 10) + (slot13 - 56320) & 63 | 128;
            }
          }
          slot5[++slot11] = slot12 ^ slot0();
          ++slot10;
        }
        slot7 = Number(slot11) + 1;
        slot5[slot7] = slot0();
        let slot14 = slot1;
        let slot15 = Number(slot2);
        slot2 = slot15 + 1;
        slot14[slot4] = slot15;
      }
      return slot7;
    }
    return slot3;
  }"###,
r###"function factory(maskInput) {
    let slot0 = maskInput;
    let slot1, slot2;
    slot1 = Object.create(null);
    slot2 = 0;
    function slot3(slot4, slot5, slot6) {
      let slot7;
      if (slot1[slot4] != null) {
        slot7 = slot6;
        slot5[++slot7] = slot0() ^ 129;
        slot7 = numberWriter(slot0, slot1[slot4], slot5, slot7);
      } else {
        let slot8 = slot4.length;
        let slot9 = 0;
        let slot10 = slot6;
        while (slot9 < slot8) {
          let slot11 = characterCode(slot4[slot9], 0);
          if (slot11 === 0) {
            slot5[++slot10] = slot0() ^ 192;
            slot11 = slot0() ^ 128;
          } else if (slot11 < 128) {
            slot11 = slot11 ^ slot0();
          } else if (slot11 < 2048) {
            slot5[++slot10] = (slot11 >> 6 | 192) ^ slot0();
            slot11 = (slot11 & 63 | 128) ^ slot0();
          } else {
            let slot12 = slot11 >= 55296 && slot11 <= 57343 && slot11 < 56320 && slot9 + 1 < slot8 ? characterCode(slot4[slot9 + 1], 0) : null;
            if (slot12 == null || slot12 < 56320 || slot12 > 57343) {
              slot5[++slot10] = (slot11 >> 12 | 224) ^ slot0();
              slot5[++slot10] = (slot11 >> 6 & 63 | 128) ^ slot0();
              slot11 = (slot11 & 63 | 128) ^ slot0();
            } else {
              ++slot9;
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 18 | 240) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 12 & 63 | 128) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 6 & 63 | 128) ^ slot0();
              slot11 = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) & 63 | 128) ^ slot0();
            }
          }
          slot5[++slot10] = slot11;
          ++slot9;
        }
        slot7 = Number(slot10) + 1;
        slot5[slot7] = slot0();
        let slot13 = slot1;
        let slot14 = Number(slot2);
        slot2 = slot14 + 1;
        slot13[slot4] = slot14;
      }
      return slot7;
    }
    return slot3;
  }"###,
r###"function factory(maskInput) {
    let slot0 = maskInput;
    let slot1, slot2;
    slot2 = Object.create(null);
    slot1 = 0;
    function slot3(slot4, slot5, slot6) {
      let slot7;
      if (slot2[slot4] != null) {
        slot7 = slot6;
        slot5[++slot7] = slot0() ^ 129;
        slot7 = numberWriter(slot0, slot2[slot4], slot5, slot7);
      } else {
        let slot8 = slot4.length;
        let slot9 = 0;
        let slot10 = slot6;
        while (slot9 < slot8) {
          let slot11 = characterCode(slot4[slot9], 0);
          if (slot11 === 0) {
            slot5[++slot10] = slot0() ^ 192;
            slot11 = slot0() ^ 128;
          } else if (slot11 < 128) {
            slot11 = slot11 ^ slot0();
          } else if (slot11 < 2048) {
            slot5[++slot10] = (slot11 >> 6 | 192) ^ slot0();
            slot11 = (slot11 & 63 | 128) ^ slot0();
          } else {
            const slot12 = slot11 >= 55296 && slot11 <= 57343 && slot11 < 56320 && slot9 + 1 < slot8 ? characterCode(slot4[slot9 + 1], 0) : null;
            if (slot12 == null || slot12 < 56320 || slot12 > 57343) {
              slot5[++slot10] = (slot11 >> 12 | 224) ^ slot0();
              slot5[++slot10] = (slot11 >> 6 & 63 | 128) ^ slot0();
              slot11 = (slot11 & 63 | 128) ^ slot0();
            } else {
              ++slot9;
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 18 | 240) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 12 & 63 | 128) ^ slot0();
              slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) >> 6 & 63 | 128) ^ slot0();
              slot11 = (65536 + (slot11 - 55296 << 10) + (slot12 - 56320) & 63 | 128) ^ slot0();
            }
          }
          slot5[++slot10] = slot11;
          ++slot9;
        }
        slot7 = Number(slot10) + 1;
        slot5[slot7] = slot0();
        let slot13 = slot2;
        let slot14 = Number(slot1);
        slot1 = slot14 + 1;
        slot13[slot4] = slot14;
      }
      return slot7;
    }
    return slot3;
  }"###,
r###"function factory(maskInput) {
    let slot0 = maskInput;
    let slot1 = Object.create(null);
    let slot2 = 0;
    function slot3(slot4, slot5, slot6) {
      let slot7;
      if (slot1[slot4] != null) {
        slot7 = slot6;
        slot7 = Number(slot6) + 1;
        slot5[slot7] = slot0() ^ 129;
        slot7 = numberWriter(slot0, slot1[slot4], slot5, slot7);
      } else {
        let slot8 = slot4.length;
        let slot9 = 0;
        let slot10 = slot6;
        while (slot9 < slot8) {
          let slot11 = characterCode(slot4[slot9], 0);
          let slot12, slot13, slot14;
          if (slot11 === 0) {
            slot5[++slot10] = slot0() ^ 192;
            slot13 = slot0();
            slot14 = 128;
          } else {
            if (slot11 < 128) {
              slot13 = slot11;
            } else if (slot11 < 2048) {
              slot5[++slot10] = (slot11 >> 6 | 192) ^ slot0();
              slot13 = slot11 & 63 | 128;
            } else {
              let slot15 = slot11 >= 55296 && slot11 <= 57343 && slot11 < 56320 && slot9 + 1 < slot8 ? characterCode(slot4[slot9 + 1], 0) : null;
              if (slot15 == null || slot15 < 56320 || slot15 > 57343) {
                slot5[++slot10] = (slot11 >> 12 | 224) ^ slot0();
                slot5[++slot10] = (slot11 >> 6 & 63 | 128) ^ slot0();
                slot13 = slot11 & 63 | 128;
              } else {
                ++slot9;
                slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot15 - 56320) >> 18 | 240) ^ slot0();
                slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot15 - 56320) >> 12 & 63 | 128) ^ slot0();
                slot5[++slot10] = (65536 + (slot11 - 55296 << 10) + (slot15 - 56320) >> 6 & 63 | 128) ^ slot0();
                slot13 = 65536 + (slot11 - 55296 << 10) + (slot15 - 56320) & 63 | 128;
              }
            }
            slot14 = slot0();
          }
          slot12 = Number(slot10) + 1;
          slot5[slot12] = slot13 ^ slot14;
          ++slot9;
          slot10 = slot12;
        }
        slot7 = Number(slot10) + 1;
        slot5[slot7] = slot0();
        let slot16 = slot1;
        let slot17 = Number(slot2);
        slot2 = slot17 + 1;
        slot16[slot4] = slot17;
      }
      return slot7;
    }
    return slot3;
  }"###
];
