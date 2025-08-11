use eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree<T> {
    nodes: Vec<T>,
    /// The edges of the tree (parent, child)
    edges: Vec<(usize, usize)>,
}

impl<T> Tree<T> {
    pub fn new(node: T) -> Self {
        Self {
            nodes: vec![node],
            edges: Vec::new(),
        }
    }

    /// Add add a new node. Returns the index of the freshly added node.
    pub fn add_node(&mut self, node: T, parent: usize) -> Result<usize> {
        if parent >= self.nodes.len() {
            eyre::bail!("Parent node does not exist");
        }
        let new_node_idx = self.nodes.len();
        self.edges.push((parent, new_node_idx));
        self.nodes.push(node);
        Ok(new_node_idx)
    }

    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn get_node(&self, idx: usize) -> &T {
        &self.nodes[idx]
    }

    pub fn get_leafs_idx(&self) -> Vec<usize> {
        let parents: std::collections::HashSet<usize> =
            self.edges.iter().map(|&(p, _)| p).collect();
        (0..self.nodes.len())
            .filter(|&idx| !parents.contains(&idx))
            .collect()
    }

    pub fn get_trajectory(&self, idx: usize) -> Vec<usize> {
        let parents: HashMap<usize, usize> = self.edges.iter().map(|&(p, c)| (c, p)).collect();
        let mut trajectory = Vec::new();
        let mut idx = idx;
        loop {
            trajectory.push(idx);
            match parents.get(&idx) {
                Some(&parent) => idx = parent,
                None => break,
            }
        }
        trajectory.reverse();
        trajectory
    }

    pub fn get_children(&self, idx: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter_map(
                |&(parent, child)| {
                    if parent == idx { Some(child) } else { None }
                },
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_order() {
        let mut tree = Tree::new(0);
        tree.add_node(1, 0).unwrap();
        let leaf = tree.add_node(2, 0).unwrap();
        let leaf = tree.add_node(3, leaf).unwrap();
        let trajectory = tree.get_trajectory(leaf);
        assert_eq!(trajectory, vec![0, 2, 3]);
    }

    #[test]
    fn test_leafs_idx() {
        let mut tree = Tree::new(0);
        tree.add_node(1, 0).unwrap();
        let leaf1 = tree.add_node(2, 0).unwrap();
        tree.add_node(3, leaf1).unwrap();
        tree.add_node(4, leaf1).unwrap();
        let leafs = tree.get_leafs_idx();
        assert_eq!(leafs, vec![1, 3, 4]);
    }
}
