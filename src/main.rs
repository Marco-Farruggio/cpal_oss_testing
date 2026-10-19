use cpal::{
    DuplexStreamConfig,
    default_host,
    SampleFormat,
    StreamConfig,
    BufferSize,
    traits::{
        StreamTrait,
        DeviceTrait,
        HostTrait,
    },
};

fn main() {
    let host = default_host();

    for (i, device) in host.devices().unwrap().enumerate() {
        let desc = device.description().unwrap();
        println!("Dev:  {i}");
        println!("Name: {}", desc.name());
        println!("Addr: {}", desc.address().unwrap_or_default());
        println!("Dir:  {}", desc.direction());
        println!("ID:   {}", device.id().unwrap());
        println!("Supports True Duplex: {}", device.supports_duplex());

        println!("Configs:");

        println!(" - In:");
        for config in device.supported_input_configs().unwrap() {
            println!(" - {config:?}");
        }

        println!(" - Out:");
        for config in device.supported_output_configs().unwrap() {
            println!(" - {config:?}");
        }

        let config = StreamConfig {
            channels: 2,
            sample_rate: 48_000,
            buffer_size: BufferSize::Default,
        };

        use std::sync::Arc;
        use std::sync::atomic::AtomicU16;

        let naudio_callbacks: Arc<AtomicU16> = Arc::new(AtomicU16::new(0));
        let cb_naudio = naudio_callbacks.clone();
        let mut p = 0f32;

        let stream = device.build_output_stream_raw(
            config,
            SampleFormat::F32,
            move |data, info| {
                cb_naudio.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                for f in data.as_slice_mut::<f32>().unwrap().chunks_mut(2) {
                    let v = p.sin() * 0.2;
                    f[0] = v;
                    if f.len() > 1 { f[1] = v; }
                    p = (p + 0.0576) % 6.283;
                }
            },
            |err| {
                eprintln!("Stream Error: {err}");
            },
            None,
        ).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));
        println!("Start 440Hz Tone");
        stream.start().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(3));
        stream.stop(None).unwrap();
        println!("End 440Hz Tone");
        println!("Number of Callbacks (~3s 48kHz 2ch): {}", naudio_callbacks.load(std::sync::atomic::Ordering::Relaxed));

        println!("Start Duplex Feedback");
        let config = DuplexStreamConfig {
            input_channels: 2,
            output_channels: 2,
            sample_rate: 48_000,
            buffer_size: BufferSize::Default,
        };

        let stream = device.build_duplex_stream_raw(
            config,
            SampleFormat::F32,
            SampleFormat::F32,
            move |input, output, info| {
                let i = input.as_slice::<f32>().unwrap();
                let o = output.as_slice_mut::<f32>().unwrap();
                for (d, s) in o.iter_mut().zip(i) {
                    *d = *s;
                }
            },
            move |err| {
                eprintln!("Stream Error: {err}");
            },
            None,
        ).unwrap();

        stream.start().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(10));
        stream.stop(None).unwrap();
        println!("End Duplex Feedback");
    }
}