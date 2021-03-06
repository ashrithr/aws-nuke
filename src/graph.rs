//! Graph utility to create DAG for tracking `Resource` dependencies.

use crate::resource::Resource;
use crate::{Error, Result};
use petgraph::{
    algo::{is_cyclic_directed, toposort},
    dot::{Config, Dot},
    stable_graph::NodeIndex,
    EdgeType, Graph,
};
use std::collections::HashMap;

#[derive(Debug, Copy, Clone)]
pub enum Relation {
    Depends,
}

pub struct Dag {
    pub graph: Graph<Resource, Relation>,
    id_map: HashMap<String, NodeIndex<u32>>,
}

impl Dag {
    /// Create a Dag with "root" node
    pub fn new() -> Self {
        let graph: Graph<Resource, Relation> = Graph::new();
        let id_map: HashMap<String, NodeIndex<u32>> = HashMap::new();

        Dag { graph, id_map }
    }

    pub fn get_dot(&self) -> Option<String> {
        if self.graph.capacity().0 > 1 {
            Some(format!(
                "{:?}",
                Dot::with_config(&self.graph, &[Config::EdgeIndexLabel])
            ))
        } else {
            None
        }
    }

    /// Add a given Resource to the DAG
    pub fn add_node_to_dag(&mut self, mut r: Resource) {
        let resource_id = r.id.clone();
        let resource_dependencies = r.dependencies.take();

        let root_index = if self.id_map.contains_key(&r.id) {
            *self.id_map.get(&r.id).unwrap()
        } else {
            let rid = self.graph.add_node(r);
            self.id_map.insert(resource_id.clone(), rid);
            rid
        };

        if let Some(dependencies) = resource_dependencies {
            for dep in dependencies {
                let dep_id = dep.id.clone();
                let dep_index = if self.id_map.contains_key(&dep.id) {
                    let tid = *self.id_map.get(&dep.id).unwrap();

                    if let Some(node) = self.graph.node_weight_mut(tid) {
                        // Replace the existing resource with dependent's state
                        node.state = dep.state;
                    }

                    tid
                } else {
                    let rid = self.graph.add_node(dep);
                    self.id_map.insert(dep_id.clone(), rid);
                    rid
                };

                self.graph
                    .add_edge(dep_index, root_index, Relation::Depends);
            }
        }
    }

    /// Order the resources based on their dependencies by performing topological
    /// sort of the DAG.
    /// TODO: Return list of lists to parallelize the execution of tasks.
    pub fn order_by_dependencies(&self) -> Result<Vec<Resource>> {
        let mut resources = Vec::new();

        match toposort(&self.graph, None) {
            Ok(order) => {
                for i in order {
                    if let Some(resource) = self.graph.node_weight(i) {
                        if resource.type_.is_default() {
                            continue;
                        }
                        resources.push(resource.clone());
                    }
                }

                Ok(resources)
            }
            Err(err) => {
                let error = self
                    .graph
                    .node_weight(err.node_id())
                    .map(|weight| format!("Error graph has cycle at node: {:?}", weight));

                Err(Error::Dag(error.unwrap_or_default()))
            }
        }
    }
}

/// Checks if provided Graph is a DAG or not
pub fn is_dag<'a, N: 'a, E: 'a, Ty, Ix>(g: &'a Graph<N, E, Ty, Ix>) -> bool
where
    Ty: EdgeType,
    Ix: petgraph::graph::IndexType,
{
    return g.is_directed() && !is_cyclic_directed(g);
}
