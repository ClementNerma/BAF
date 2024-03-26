use std::fmt::Display;

pub enum Diagnostic {
    DirTakesExistingName {
        parent_dir_id: Option<u64>,
        dir_id: u64,
        name: String,
    },

    FileTakesExistingName {
        parent_dir_id: Option<u64>,
        file_id: u64,
        name: String,
    },
}

impl Diagnostic {
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

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Low,
    Medium,
    High,
}
