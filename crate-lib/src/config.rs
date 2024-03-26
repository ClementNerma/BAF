pub struct Config {
    pub default_dirs_capacity: u32,
    pub default_files_capacity: u32,
    pub first_segment_files_capacity_override: Option<u32>,
    pub first_segment_dirs_capacity_override: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_dirs_capacity: 1024,
            default_files_capacity: 1024,
            first_segment_files_capacity_override: None,
            first_segment_dirs_capacity_override: None,
        }
    }
}
