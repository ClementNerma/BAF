use std::{
    fmt::Display,
    io::{Read, Seek},
};

use baf::{Archive, DirectoryId as DirId, DirectoryIdOrRoot};

pub struct ArchiveContentTree {
    root_nodes: Vec<TreeNode>,
}

impl ArchiveContentTree {
    /// Build the tree structure from directory map and file list
    pub fn build(archive: &Archive<impl Read + Seek>) -> Self {
        // Build root nodes
        let mut root_nodes = Vec::new();

        let (dir_ids, file_ids) = archive.get_dir_content(DirectoryIdOrRoot::Root).unwrap();

        // Add root directories
        for dir_id in dir_ids {
            root_nodes.push(build_dir_node(*dir_id, archive));
        }

        // Add root files
        for file_id in file_ids {
            root_nodes.push(TreeNode::new_file(
                archive
                    .get_file(*file_id)
                    .unwrap()
                    .name
                    .clone()
                    .into_string(),
            ));
        }

        // Sort roots
        root_nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        Self { root_nodes }
    }
}

// Recursive function to build tree nodes
fn build_dir_node(dir_id: DirId, archive: &Archive<impl Read + Seek>) -> TreeNode {
    let mut node = TreeNode::new_dir(archive.get_dir(dir_id).unwrap().name.clone().into_string());

    let (dir_ids, file_ids) = archive
        .get_dir_content(DirectoryIdOrRoot::NonRoot(dir_id))
        .unwrap();

    // Add subdirectories
    for dir_id in dir_ids {
        node.children.push(build_dir_node(*dir_id, archive));
    }

    // Add files
    for file_id in file_ids {
        node.children.push(TreeNode::new_file(
            archive
                .get_file(*file_id)
                .unwrap()
                .name
                .clone()
                .into_string(),
        ));
    }

    // Sort: directories first, then files, alphabetically within each group
    node.children.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    node
}

impl Display for ArchiveContentTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { root_nodes } = self;

        // Render the tree to a string with nice box-drawing characters
        writeln!(f, ".")?;

        for (i, node) in root_nodes.iter().enumerate() {
            let is_last = i == root_nodes.len() - 1;
            render_node(node, f, "", is_last, i == 0)?;
        }

        Ok(())
    }
}

fn render_node(
    node: &TreeNode,
    f: &mut std::fmt::Formatter<'_>,
    prefix: &str,
    is_last: bool,
    is_tree_first: bool,
) -> std::fmt::Result {
    let connector = if is_last { "└── " } else { "├── " };
    let suffix = if node.is_dir { "/" } else { "" };

    if !is_tree_first {
        writeln!(f)?;
    }

    write!(f, "{prefix}")?;
    write!(f, "{connector}")?;
    write!(f, "{}", node.name)?;
    write!(f, "{suffix}")?;

    let extension = if is_last { "    " } else { "│   " };
    let new_prefix = format!("{}{}", prefix, extension);

    for (i, child) in node.children.iter().enumerate() {
        let child_is_last = i == node.children.len() - 1;
        render_node(child, f, &new_prefix, child_is_last, false)?;
    }

    Ok(())
}

#[derive(Debug)]
struct TreeNode {
    name: String,
    is_dir: bool,
    children: Vec<TreeNode>,
}

impl TreeNode {
    fn new_dir(name: String) -> Self {
        Self {
            name,
            is_dir: true,
            children: Vec::new(),
        }
    }

    fn new_file(name: String) -> Self {
        Self {
            name,
            is_dir: false,
            children: Vec::new(),
        }
    }
}
