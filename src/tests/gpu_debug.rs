use std::fmt::Debug;
pub fn read_buffer_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    byte_offset: u64,
    byte_len: u64,
) -> Result<Vec<u8>, wgpu::MapRangeError> {
    assert!(
        byte_offset + byte_len <= buffer.size(),
        "read range exceeds buffer size {}",
        buffer.size()
    );
    // copy_buffer_to_buffer requires a 4-byte-aligned size
    let padded_len = byte_len.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT);

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu_debug readback staging"),
        size: padded_len,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("gpu_debug readback encoder"),
    });
    encoder.copy_buffer_to_buffer(buffer, byte_offset, &staging, 0, padded_len);
    queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    // PollType::Wait blocks until all pending map callbacks have fired.
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device.poll(Wait) failed");
    rx.recv()
        .expect("map callback channel dropped")
        .expect("map_async failed");

    let view = slice.get_mapped_range()?;
    let out = view[..byte_len as usize].to_vec();
    drop(view);
    staging.unmap();
    Ok(out)
}

/// Read `count` values of `T` starting at element index `elem_offset`.
pub fn read_buffer<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    elem_offset: u64,
    count: usize,
) -> Result<Vec<T>, wgpu::MapRangeError> {
    let elem = std::mem::size_of::<T>() as u64;
    let bytes = read_buffer_bytes(
        device,
        queue,
        buffer,
        elem_offset * elem,
        count as u64 * elem,
    )?;
    Ok(bytemuck::cast_slice::<u8, T>(&bytes).to_vec())
}

/// Read and reinterpret the entire buffer as `Vec<T>`.
pub fn read_whole_buffer<T: bytemuck::Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
) -> Result<Vec<T>, wgpu::MapRangeError> {
    let count = (buffer.size() / std::mem::size_of::<T>() as u64) as usize;
    let res = read_buffer::<T>(device, queue, buffer, 0, count)?;
    Ok(res)
}

/// Read `count` values of `T` and pretty-print them with their indices.
pub fn dump_buffer<T: bytemuck::Pod + Debug>(
    label: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    elem_offset: u64,
    count: usize,
) {
    let vals = read_buffer::<T>(device, queue, buffer, elem_offset, count);
    println!(
        "── GPU buffer: {label}  ({count} × {}, {} bytes each) ──",
        std::any::type_name::<T>(),
        std::mem::size_of::<T>(),
    );
    for (i, v) in vals.iter().enumerate() {
        println!("  [{:>4}] {v:?}", elem_offset as usize + i);
    }
}
