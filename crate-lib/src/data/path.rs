use std::{
    fmt::Display,
    path::{Component, Path},
};

use anyhow::{Result, anyhow};

use super::name::{ItemName, NameValidationError};

// TODO: this is not super efficient as this requires lots of unnecessary allocations
pub struct PathInArchive(Vec<ItemName>);

impl PathInArchive {
    /// Split a path as a list of components
    ///
    /// Handles `.` and `..` symbol, prevents escapes from root
    ///
    /// Does not preserve the root symbol (`/` at the beginning of a path)
    pub fn new(path: &str) -> Result<Self> {
        let mut out = vec![];

        for component in Path::new(path).components() {
            match component {
                Component::Prefix(_) | Component::RootDir | Component::CurDir => {}

                Component::ParentDir => {
                    out.pop();
                }

                Component::Normal(normal) => {
                    let str = normal.to_str().unwrap();

                    out.push(ItemName::new(str.to_owned()).map_err(|err| {
                        anyhow!("In path '{path}': component '{str}' is invalid: {err}")
                    })?);
                }
            }
        }

        Ok(Self(out))
    }

    /// Create a path from a suite of components
    /// TODO: unnecessarily allocates
    pub fn from_components(components: &[&str]) -> Result<Self> {
        Self::new(&components.join("/"))
    }

    /// Create an empty path
    pub fn empty() -> Self {
        Self(vec![])
    }

    /// Get the list of components
    ///
    /// Guaranteed to only contain only valid names (see [`ItemName`])
    pub fn components(&self) -> &[ItemName] {
        &self.0
    }

    /// Check if the path is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the parent directory
    pub fn parent(&self) -> Option<Self> {
        if self.components().len() < 2 {
            None
        } else {
            let mut components = self.components().to_vec();
            components.pop();
            Some(Self(components))
        }
    }

    /// Get the filename
    pub fn filename(&self) -> Option<&ItemName> {
        self.components().last()
    }

    /// Pop the last component
    pub fn pop(&mut self) -> Option<ItemName> {
        self.0.pop()
    }

    /// Append a new component
    pub fn append(&mut self, component: ItemName) {
        self.0.push(component);
    }

    /// Append a new string component
    pub fn append_str(&mut self, component: impl Into<String>) -> Result<(), NameValidationError> {
        let name = ItemName::new(component.into())?;
        self.append(name);
        Ok(())
    }

    /// Append a new component and return the new path
    pub fn join(mut self, component: ItemName) -> Self {
        self.append(component);
        self
    }

    /// Append a new string component and return the new path
    pub fn join_str(mut self, component: impl Into<String>) -> Result<Self, NameValidationError> {
        self.append_str(component)?;
        Ok(self)
    }
}

impl Display for PathInArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, comp) in self.0.iter().enumerate() {
            write!(f, "{}{comp}", if i == 0 { "" } else { "/" })?;
        }

        Ok(())
    }
}
