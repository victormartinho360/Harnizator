//! Layout do grafo de agentes: camadas por profundidade (spec/06, D18).

use std::collections::HashMap;

use harness_core::AgentId;
use harness_core::agents::NodeView;

/// Largura/altura de um nó no grafo.
pub const NODE_W: u16 = 14;
pub const NODE_H: u16 = 3;

/// Posições computadas dos nós (coords em células de terminal).
pub struct GraphLayout {
    positions: HashMap<AgentId, Pos>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub x: u16,
    pub y: u16,
}

impl GraphLayout {
    /// Computa posições: filhos centralizados sob o pai, espalhados na camada.
    pub fn compute(nodes: &[NodeView], width: u16, _height: u16) -> Self {
        // agrupa por profundidade
        let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
        let mut positions = HashMap::new();
        for depth in 0..=max_depth {
            let layer: Vec<&NodeView> = nodes.iter().filter(|n| n.depth == depth).collect();
            let count = layer.len() as u16;
            if count == 0 {
                continue;
            }
            // espaçamento uniforme na largura disponível
            let span = width.saturating_sub(NODE_W);
            for (i, node) in layer.iter().enumerate() {
                let x = if count == 1 {
                    width.saturating_sub(NODE_W) / 2
                } else {
                    span * (i as u16) / (count - 1)
                };
                let y = depth as u16 * (NODE_H + 1);
                positions.insert(node.id.clone(), Pos { x, y: y + 1 });
            }
        }
        Self { positions }
    }

    pub fn position(&self, id: &AgentId) -> Option<Pos> {
        self.positions.get(id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AgentId, &Pos)> {
        self.positions.iter()
    }
}
