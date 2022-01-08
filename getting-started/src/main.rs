use std::borrow::Cow;
use wgpu::util::DeviceExt;

fn main() {
    pollster::block_on(async_main());
}

async fn async_main() {
    // Boilerplate initialisation of WGPU

    // Instantiating wgpu
    let instance = wgpu::Instance::new(wgpu::Backends::all());

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptionsBase::default())
        .await
        .expect("No GPU Found for referenced preference");

    // `request_device` instantiates the feature specific connection to the GPU, defining some parameters,
    //  `features` being the available features.
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .expect("Could not create adapter for GPU device");

    // USER INPUT

    // let x = [1.0f32, 2.0, 3.0, 4.0, 5.0];
    let x: Vec<f32> = (0..1028).map(|x| x as f32).collect();

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

    let (x_workgroups, y_workgroups, z_workgroups) = (x.len() as u32, 1, 1);

    // Boilerplate WGPU input

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("x"),
        contents: bytemuck::cast_slice(&x),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (x.len() * std::mem::size_of::<f32>()) as _,
        mapped_at_creation: false,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ,
    });

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

    // Boilerplate WGPU compute

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);

        cpass.dispatch(x_workgroups, y_workgroups, z_workgroups); // Number of cells to run, the (x,y,z) size of item being processed
    }
    queue.submit(Some(encoder.finish()));

    // Boilerplate retrieving output

    let buffer_slice = y_buffer.slice(..);
    let buffer_future = buffer_slice.map_async(wgpu::MapMode::Read);

    device.poll(wgpu::Maintain::Wait);

    buffer_future.await.expect("failed to run compute on gpu!");
    // Gets contents of buffer
    let data = buffer_slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    y_buffer.unmap();

    let expected_result: Vec<f32> = (0..5).map(|x| f32::cos(x as _)).collect();

    println!("Result: {:?}", &result[0..5]);
    println!("expected result: {:?}", &expected_result);
    println!("Result length: {:?}", &result.len());
}
