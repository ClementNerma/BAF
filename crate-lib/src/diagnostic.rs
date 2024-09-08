use std::{fmt::Display, ops::Deref};

use crate::data::name::{ItemName, NameDecodingError};

/// Diagnostic emitted while checking an archive's correctness
pub enum Diagnostic {
    /// A directory takes a name that's already used
    ItemHasDuplicateName {
        is_dir: bool,
        item_id: u64,
        parent_dir_id: Option<u64>,
        name: ItemName,
    },

    /// Item has invalid name
    InvalidItemName {
        is_dir: bool,
        ft_entry_addr: u64,
        error: NameDecodingError,
    },
}

impl Diagnostic {
    /// Get the severity of a given diagnostic
    pub fn severity(&self) -> Severity {
        match self {
            Diagnostic::ItemHasDuplicateName {
                parent_dir_id: _,
                is_dir: _,
                item_id: _,
                name: _,
            } => Severity::Medium,

            Diagnostic::InvalidItemName {
                is_dir: _,
                ft_entry_addr: _,
                error: _,
            } => Severity::High,
        }
    }
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Diagnostic::ItemHasDuplicateName {
                parent_dir_id,
                is_dir,
                item_id,
                name,
            } => {
                write!(
                    f,
                    "{} with ID {item_id} is using a duplicate name '{}' in {}",
                    if *is_dir { "Directory" } else { "File" },
                    name.deref(),
                    match parent_dir_id {
                        Some(id) => format!("parent directory with ID {id}"),
                        None => "root directory".to_owned(),
                    }
                )
            }

            Diagnostic::InvalidItemName {
                is_dir,
                ft_entry_addr,
                error,
            } => {
                write!(
                    f,
                    "File table entry at address {ft_entry_addr} represents {} with an invalid name: {}",
                    if *is_dir { "directory" } else { "file" },
                    error.cause
                )
            }
        }
    }
}

/// Severity of a diagnostic
#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Low,
    Medium,
    High,
}
