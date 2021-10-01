//! Execution transformation logs.

use casper_types::{
    ExecutionEffect as JsonExecutionEffect, Key, TransformEntry as JsonTransformEntry,
};

use crate::shared::transform::Transform;

/// A log of all transforms produced during execution.
#[derive(Debug, Default, Clone, derive_more::From, derive_more::Into)]
pub struct ExecutionJournal(Vec<(Key, Transform)>);

impl ExecutionJournal {
    /// Whether the journal is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many transforms are recorded in the journal.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Adds a transform to the journal.
    pub fn push(&mut self, entry: (Key, Transform)) {
        self.0.push(entry)
    }
}

impl From<ExecutionJournal> for JsonExecutionEffect {
    fn from(execution_journal: ExecutionJournal) -> Self {
        Self::new(
            <Vec<(Key, Transform)>>::from(execution_journal)
                .iter()
                .map(|(key, transform)| JsonTransformEntry {
                    key: key.to_formatted_string(),
                    transform: transform.into(),
                })
                .collect(),
        )
    }
}
