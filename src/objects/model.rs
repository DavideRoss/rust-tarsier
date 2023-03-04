use std::io::BufReader;
use std::fs::File;
use std::collections::HashMap;

use nalgebra_glm as glm;

use crate::Vertex;

pub struct Model {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>
}

impl Model {
    pub fn from_file(filepath: &str) -> Self {
        let mut reader = BufReader::new(File::open(filepath).unwrap());

        let (models, _) = tobj::load_obj_buf(
            &mut reader,
            &tobj::LoadOptions {
                triangulate: true,
                ..Default::default()
            },
            |_| Ok(Default::default())
        ).unwrap();

        let mut vertices = vec![];
        let mut indices = vec![];

        let mut unique_vertices = HashMap::new();

        for model in models {
            for index in &model.mesh.indices {
                let pos_offset = (3 * index) as usize;
                let uv_offset = (2 * index) as usize;

                let vertex = Vertex::new(
                    glm::vec3(
                        model.mesh.positions[pos_offset],
                        model.mesh.positions[pos_offset + 1],
                        model.mesh.positions[pos_offset + 2],
                    ),
                    glm::vec2(
                        model.mesh.texcoords[uv_offset],
                        1.0 - model.mesh.texcoords[uv_offset + 1]
                    )
                );

                if let Some(index) = unique_vertices.get(&vertex) {
                    indices.push(*index as u32);
                } else {
                    let index = vertices.len();
                    unique_vertices.insert(vertex, index);
                    vertices.push(vertex);
                    indices.push(index as u32);
                }
            }
        }

        Model {
            vertices,
            indices
        }
    }
}