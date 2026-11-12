//! Structural readers of one recovered `program.js`: lexical bindings, the
//! descriptor scanner, collector wiring, encoding helpers, per-signal emission
//! layouts, generator inputs and the generator-ready signal contracts.
//!
//! ```no_run
//! let source = std::fs::read_to_string("program.js")?;
//! let properties = shape::signals::extract::extract_program_properties(&source)?;
//! println!("{}", serde_json::to_string_pretty(&properties)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod bindings;
pub mod contracts;
pub mod encoding;
pub mod generator_inputs;
pub mod host_probes;
pub mod inputs;
pub mod layout;
pub(crate) mod reaching;
pub mod scanner;
pub mod wiring;

pub use inputs::{
    extract_program_inputs, ProgramInputError, ProgramInputReport, PROGRAM_INPUT_SCHEMA_VERSION,
};
pub use scanner::{extract_program_properties, extract_program_properties_with_limits};
