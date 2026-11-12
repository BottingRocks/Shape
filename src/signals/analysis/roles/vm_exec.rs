//! The execution-framework finder packages: the reactive operators, their
//! executors, observers and teardowns (`operators`, package `p02_exec_a`) and
//! the clock, task-queue helpers, reactive classes, batch scheduler and
//! callback notifier (`classes`, package `p03_exec_b`).

pub(crate) mod classes;
pub(crate) mod operators;
