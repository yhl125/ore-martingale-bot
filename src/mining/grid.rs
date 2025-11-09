use rand::seq::SliceRandom;
use rand::rng;

pub const GRID_SIZE: usize = 5;
pub const TOTAL_BLOCKS: usize = GRID_SIZE * GRID_SIZE; // 25

#[derive(Debug, Clone, Copy)]
pub struct BlockPosition {
    pub row: u8,    // 0-4
    pub col: u8,    // 0-4
    pub index: u8,  // 0-24 (row * 5 + col)
}

impl BlockPosition {
    pub fn from_index(index: u8) -> Self {
        assert!(index < TOTAL_BLOCKS as u8);
        Self {
            row: index / GRID_SIZE as u8,
            col: index % GRID_SIZE as u8,
            index,
        }
    }
}

/// Select blocks to bet on randomly
pub fn select_blocks(count: u8) -> Vec<BlockPosition> {
    let count = (count as usize).min(TOTAL_BLOCKS);
    
    let mut rng = rng();
    let mut indices: Vec<u8> = (0..TOTAL_BLOCKS as u8).collect();

    indices.shuffle(&mut rng);

    indices.into_iter()
        .take(count)
        .map(BlockPosition::from_index)
        .collect()
}
