# Source-derived signal facts

These readers recover facts from JavaScript without executing it. They feed the
recognition stage in the [parent subsystem](../README.md); they do not assign
canonical signal names or encode payload bytes.

| File | Start here for |
| --- | --- |
| [mod.rs](mod.rs), [scanner.rs](scanner.rs) | Property extraction entrypoints, limits and registry/emission scanning |
| [bindings.rs](bindings.rs) | Lexical function/binding resolution and parameter slots |
| [wiring.rs](wiring.rs) | Collector routes, observer wiring and lifecycle roles |
| [encoding.rs](encoding.rs) | Structural identities of encoding helpers |
| [inputs.rs](inputs.rs) | `extract_program_inputs`, the composition and source-identity checks |
| [layout.rs](layout.rs) | Ordered, emission-scoped buffer writes, prefixes and masks |
| [generator_inputs.rs](generator_inputs.rs) | Glyph and DOM-census semantics projected from layouts |
| [contracts.rs](contracts.rs) | Typed grammar, provenance, lifecycle and write accounting |

Composition obtains encoding and wiring facts for the same source, builds the
layout request from their helper/collector identities, then derives generator
inputs. Spans are joins within that source, not reusable coordinates for another
program. The scanner also integrates layout gaps into its unresolved facts.

Layout reconstruction snapshots writes at an emission on its execution path;
it does not union all writes in the surrounding function. Callback bodies keep
their own write scope. Unknown calls and unsupported paths remain explicit gaps.
Contract status distinguishes complete, partial and unsupported results, so the
presence of a record alone is not proof of a complete generator contract.

[reaching.rs](reaching.rs) is an internal, deliberately narrow value-at-use
proof for reused locals: it requires a preceding plain definition in the same
statement list and refuses competing nested writes and unsupported control flow.
Do not substitute binding identity for the value held at a particular read.

Read the [contract reference](../../../docs/signals/contracts.md) and
[program_signal_extraction.rs](../../../tests/program_signal_extraction.rs) for
consumer expectations; focused unit tests also live beside the extractors.
