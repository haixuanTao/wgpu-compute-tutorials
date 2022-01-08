# GPU Computation in Rust with WGPU

This post is a bare minimal "getting started" for GPU computation in Rust with wgpu using WGSL.

It is going to compute cosine for many integers in parallel. 

Big Kudos to the wgpu team for this great crate, the WGSL and WebGPU workgroup for their immense work.

## 0. Install a backend for wgpu to connect to

If you're on linux, you're going to need to install vulkan: https://linuxconfig.org/install-and-test-vulkan-on-linux

If you're on Windows, you should have DX12, and this will be your backend.

On Mac, I haven't tried but, you should install Metal if it is not already installed.

## 1. Declare some dependencies


```toml
wgpu = "0.12.0"
bytemuck = "1.7.2"
pollster = "0.2.4"
```

- wgpu: send computation to the GPU
- bytemuck: format from f32 to u8 and back.
- pollster: Enable to wait for promises within sync function.

## 2. implement an async function 

Wgpu does asynchronous calls to the gpu. So use an async function.

```rust
fn main() {
    pollster::block_on(async_main());
}

async fn async_main() {
```

## 3. Connect to the GPU

Create a connection using the following boilerplate within the async function:

```rust
    let instance = wgpu::Instance::new(wgpu::Backends::all());

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptionsBase::default())
        .await
        .expect("No GPU Found for referenced preference");
    
		let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .expect("Could not create adapter for GPU device");
```

- instance is the instantiation of wgpu. You can define the backend you want to use (Metal/DX12/Vulkan or default)
- adapter is a handler to the gpu to create a connection. You can specify the GPU you want to use in case there is several.
- device and queue are the connection that is going to be used to pass data and compute pipeline.

***You can try to compile the code at this point to check if your hardware is working fine. If not, check that the backend is well installed.***

## 4. Create some data and send it to the GPU

A buffer is what connects data from the CPU to the GPU. We are going to create one for our input `x`:

```rust
    let x: Vec<f32> = (0..1028).map(|x| x as f32).collect();

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("x"),
        contents: bytemuck::cast_slice(&x),
        usage: wgpu::BufferUsages::STORAGE,
    });
```

- label is the name of the buffer for the logs within the GPU computation for debugging.
- contents are the data of the buffer. It has to be a `&[u8]` and you can convert `&[f32]` to `&[u8]` using bytemuck.
- usage defines the permission granted to the buffer.

You will need to add the following at the beginning of `main.rs`:

```rust
use wgpu::util::DeviceExt;
```

You can also create a buffer without initialising its data as follows with our output `y`:

```rust
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("y"),
        size: (x.len() * std::mem::size_of::<f32>()) as _,
        mapped_at_creation: false,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ,
    });
```
- size is the memory size of the buffer. In this case, as we're not modifying the size of the output, we're going to copy the length of x. But as x is `f32` and the buffer is in `u8` we have to multiply by the size of `f32`.


## 5. Write a WGSL Shader

Wgpu allows you to use `SPIR-V`, `GLSL`, `WGSL` shader language to write the program you want to run on the gpu.

I am going to use WGSL: 
```rust
    let shader = "
    
struct Array {
    data: [[stride(4)]] array<f32>;
}; 

[[group(0), binding(0)]]
var<storage, read> x: Array;

[[group(0), binding(1)]]
var<storage, write> y: Array;
    
[[stage(compute), workgroup_size(1, 1, 1)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    let gidx = global_id.x;
    y.data[gidx] = cos(x.data[gidx]);
}
    ";
```

In WGSL, you will need a Struct to parse the array of `u8` to make them an array of `f32`. You will also have to add the stride between each element of the array.  

Each buffer is referenced by a bind group index and a binding index. In my case, I want to associate `[[group(0), binding(0)]]` to buffer `x` and `[[group(0), binding(1)]]` to buffer `y`.

## 6. Create a Compute Pipeline and a bind group.

```rust
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader)),
        }),
        entry_point: "main",
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: x_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: y_buffer.as_entire_binding(),
            },
        ],
    });
```

Create a compute pipeline from the shader and a bindgroup that is following the layout that we specified within the shader.

## 6. Dispatch it to the GPU!
This boilerplate code will glue everything together and dispatch it to the gpu.

```rust
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);

        cpass.dispatch(x_workgroups, y_workgroups, z_workgroups); 
    }
    queue.submit(Some(encoder.finish()));
```


## 7. Retrieve the output

```rust
    let buffer_slice = y_buffer.slice(..);
    let buffer_future = buffer_slice.map_async(wgpu::MapMode::Read);

    device.poll(wgpu::Maintain::Wait);

    buffer_future.await.expect("failed to run compute on gpu!");
    let data = buffer_slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    y_buffer.unmap();
```

Because everything thing is async, you have to await that the computation is done and the buffer can be read. 

Unmapping the result will allow you to reuse the buffer.

## 8. Check the result

```rust
    let expected_result: Vec<f32> = (0..5).map(|x| f32::cos(x as _)).collect();

    println!("Result: {:?}", &result[0..5]);
    println!("expected result: {:?}", &expected_result);

}
```

```bash
RUST_LOG=info cargo run
```
```
warning: unused manifest key: package.author
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running `target/debug/getting-started`
MESA-INTEL: warning: Performance support disabled, consider sysctl dev.i915.perf_stream_paranoid=0

Result: [1.0, 0.5403116, -0.41615236, -0.9900058, -0.65365756]
expected result: [0.5403023, -0.41614684, -0.9899925, -0.6536436, 0.2836622]
Result length: 1028
```

