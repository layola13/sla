struct BlobRange {
    bytes: i32,
    chunk_size: i32,
}

fn chunk_layout(blob: BlobRange) -> i32 {
    let chunks = (blob.bytes + blob.chunk_size - 1) / blob.chunk_size;
    let tail = blob.bytes % blob.chunk_size;
    chunks * 10 + tail
}

fn main() {
    println!("{}", chunk_layout(BlobRange { bytes: 10, chunk_size: 4 }));
}
