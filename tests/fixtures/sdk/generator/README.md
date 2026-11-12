# Generator and codec regression fixtures

Synthetic data retained from the former repository generator and codec
demonstrations. Fixed keys, seeds and tokens are deterministic test inputs.

- profile.json and session-profile.json configure generator roundtrips.
- raw.json and container.json cover raw bytes and opaque channel sections.
- report.json covers missing entries, duplicate keyed IDs and special values.
- superpack-values.json and superpack-extensions.json cover typed packing.
- signal-codec.json supplies three layout/value pairs for codec unit tests.

Consumers are the generator_cli, generator_header_report, generator_headers,
generator_pack_cli, generator_pipeline and generator_report integration tests,
and signals::codec::layout unit tests. No fixture is a runnable public example.
