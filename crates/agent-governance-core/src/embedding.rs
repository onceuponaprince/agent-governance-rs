use sha2::{Digest, Sha256};

/// Deterministic, local embedding prototype.
/// Produces `dim`-length f32 vector from input text by repeated SHA256 hashing.
pub fn compute_embedding(text: &str, dim: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(dim);
    let mut counter: u64 = 0;
    while out.len() < dim {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        hasher.update(counter.to_le_bytes());
        let digest = hasher.finalize();
        // digest is 32 bytes; convert into 8 f32 values (4 bytes each) until we fill
        for chunk in digest.chunks(4) {
            if out.len() >= dim {
                break;
            }
            let mut arr = [0u8; 4];
            for (i, b) in chunk.iter().enumerate() {
                arr[i] = *b;
            }
            // interpret as u32 then map to [-1.0, 1.0]
            let as_u = u32::from_le_bytes(arr);
            let f = (as_u as f32) / (u32::MAX as f32);
            let mapped = f * 2.0 - 1.0;
            out.push(mapped);
        }
        counter += 1;
    }
    out.truncate(dim);
    out
}
