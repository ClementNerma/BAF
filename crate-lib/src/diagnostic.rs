use std::fmt::Display;

/// Diagnostic emitted while checking an archive's correctness
pub enum Diagnostic {
    /// A directory takes a name that's already used
    DirTakesExistingName {
        parent_dir_id: Option<u64>,
        dir_id: u64,
        name: String,
    },

    /// A file takes a name that's already use
    FileTakesExistingName {
        parent_dir_id: Option<u64>,
        file_id: u64,
        name: String,
    },
}

impl Diagnostic {
    /// Get the severity of a given diagnostic
    pub fn severity(&self) -> Severity {
        match self {
            Diagnostic::DirTakesExistingName {
                parent_dir_id: _,
                dir_id: _,
                name: _,
            } => Severity::Medium,

            Diagnostic::FileTakesExistingName {
                parent_dir_id: _,
                file_id: _,
                name: _,
            } => Severity::Medium,
        }
    }
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Diagnostic::DirTakesExistingName {
                parent_dir_id,
                dir_id,
                name,
            } => {
                write!(
                    f,
                    "Directory with ID {dir_id} is using a duplicate name '{name}' in ",
                )?;

                match parent_dir_id {
                    Some(parent_dir_id) => write!(f, "parent directory with ID {parent_dir_id}"),
                    None => write!(f, "root directory"),
                }
            }

            Diagnostic::FileTakesExistingName {
                parent_dir_id,
                file_id,
                name,
            } => {
                write!(
                    f,
                    "File with ID {file_id} is using a duplicate name '{name}' in ",
                )?;

                match parent_dir_id {
                    Some(parent_dir_id) => write!(f, "parent directory with ID {parent_dir_id}"),
                    None => write!(f, "root directory"),
                }
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
