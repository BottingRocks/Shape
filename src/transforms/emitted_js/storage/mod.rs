//! Declarations, slots and their storage.

pub(crate) mod last_use_storage;
pub(crate) mod fresh_values;
pub(crate) mod private_array_reads;
pub(crate) mod declaration_lifetimes;
pub(crate) mod dead_locals;
pub(crate) mod dead_key;
pub(crate) mod local_vars;
pub(crate) mod unused_keys;
pub(crate) mod key_declarations;
pub(crate) mod key_webs;
pub(crate) mod redundant_initializers;
pub(crate) mod unread_declarations;
