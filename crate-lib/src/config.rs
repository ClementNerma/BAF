/// Configuration of an archive's behaviour
pub struct ArchiveConfig {
    /// Maximum number of directory entries per file table segment
    pub default_dirs_capacity_by_ft_segment: u32,

    /// Maximum number of file entries per file table segment
    pub default_files_capacity_by_ft_segment: u32,

    /// Override the maximum number of directory entries for the first table segment
    pub first_segment_dirs_capacity_override: Option<u32>,

    /// Override the maximum number of file entries for the first table segment
    pub first_segment_files_capacity_override: Option<u32>,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            default_dirs_capacity_by_ft_segment: 1024,
            default_files_capacity_by_ft_segment: 1024,
            first_segment_files_capacity_override: None,
            first_segment_dirs_capacity_override: None,
        }
    }
}
