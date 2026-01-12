use std::num::NonZero;

/// Configuration of an archive's behaviour
pub struct ArchiveConfig {
    /// Maximum number of directory entries per file table segment
    pub default_dirs_capacity_by_ft_segment: NonZero<u32>,

    /// Maximum number of file entries per file table segment
    pub default_files_capacity_by_ft_segment: NonZero<u32>,

    /// Override the maximum number of directory entries for the first table segment
    pub first_segment_dirs_capacity_override: Option<NonZero<u32>>,

    /// Override the maximum number of file entries for the first table segment
    pub first_segment_files_capacity_override: Option<NonZero<u32>>,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            default_dirs_capacity_by_ft_segment: NonZero::new(100).unwrap(),
            default_files_capacity_by_ft_segment: NonZero::new(100).unwrap(),
            first_segment_files_capacity_override: Some(NonZero::new(10).unwrap()),
            first_segment_dirs_capacity_override: Some(NonZero::new(10).unwrap()),
        }
    }
}
