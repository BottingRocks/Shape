# Decode regression fixtures

Synthetic traced JavaScript, parsed session values and typed capture headers retained
from the former repository decode demonstration. The program uses the reference
hash, stream and number writers in src/signals/wire/reference; its independent
golden bytes are described in ../../signal-wire-roundtrip/README.md.

The decode_workflow integration tests compare these files with independently
constructed inputs. These are test data, not public example programs or real
captures. Run: cargo test --release --test decode_workflow.
