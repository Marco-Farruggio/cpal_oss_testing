use std::{
    fmt::Display,
    sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    },
    thread::sleep,
    time::Duration,
};

use cpal::{
    DuplexStreamConfig,
    SampleFormat,
    StreamConfig,
    default_host,
    BufferSize,
    Device,
    traits::{
        StreamTrait,
        DeviceTrait,
        HostTrait,
    },
};

/// Print a non-fatal error in a consistent format.
fn report(context: &str, err: impl Display) {
    eprintln!("  [!] {context}: {err}");
}

fn main() {
    let host = default_host();

    let devices = match host.devices() {
        Ok(devices) => devices,
        Err(e) => {
            report("Failed to enumerate devices", e);
            return;
        }
    };

    for (i, device) in devices.enumerate() {
        println!("Dev:  {i}");

        print_device_info(&device);
        test_tone(&device);
        // test_input(&device); [TODO]
        test_duplex(&device);
    }
}

fn print_device_info(device: &Device) {
    match device.description() {
        Ok(desc) => {
            println!("Name: {}", desc.name());
            println!("Addr: {}", desc.address().unwrap_or_default());
            println!("Dir:  {}", desc.direction());
        }
        Err(e) => report("Failed to get device description", e),
    }

    match device.id() {
        Ok(id) => println!("ID:   {id}"),
        Err(e) => report("Failed to get device ID", e),
    }

    println!("Supports True Duplex: {}", device.supports_duplex());

    println!("Configs:");

    println!(" - In:");
    match device.supported_input_configs() {
        Ok(configs) => {
            for config in configs {
                println!(" - {config:?}");
            }
        }
        Err(e) => report("Failed to query supported input configs", e),
    }

    println!(" - Out:");
    match device.supported_output_configs() {
        Ok(configs) => {
            for config in configs {
                println!(" - {config:?}");
            }
        }
        Err(e) => report("Failed to query supported output configs", e),
    }
}

fn test_tone(device: &Device) {
    println!("[O] Starting Output Tone Test ...");
    let config = StreamConfig {
        channels: 2,
        sample_rate: 48_000,
        buffer_size: BufferSize::Default,
    };

    let naudio_callbacks = Arc::new(AtomicU16::new(0));
    let cb_naudio = naudio_callbacks.clone();
    let mut phase = 0f32;

    println!("[O] Building stream ...");
    let stream = match device.build_output_stream_raw(
        config,
        SampleFormat::F32,
        move |data, _info| {
            cb_naudio.fetch_add(1, Ordering::Relaxed);

            for frame in data.as_slice_mut::<f32>().unwrap().chunks_mut(2) {
                let v = phase.sin() * 0.2;
                frame[0] = v;
                if frame.len() > 1 {
                    frame[1] = v;
                }
                phase = (phase + 0.0576) % 6.283;
            }
        },
        |err| {
            eprintln!("[O] Stream Error: {err}");
        },
        None,
    ) {
        Ok(stream) => stream,
        Err(e) => {
            report("[O] Failed to build stream", e);
            return;
        }
    };

    sleep(Duration::from_secs(2));

    println!("[O] Starting stream");
    if let Err(e) = stream.start() {
        report("[O] Failed to start stream", e);
        return;
    }

    sleep(Duration::from_secs(3));

    println!("[O] Stopping stream ...");
    if let Err(e) = stream.stop(None) {
        report("[O] Failed to stop stream", e);
    }
    println!("[O] Number of Callbacks (~3s 48kHz 2ch): {}", naudio_callbacks.load(Ordering::Relaxed));
}

fn test_duplex(device: &Device) {
    println!("[D] Starting Duplex Feedback Test ...");

    let config = DuplexStreamConfig {
        input_channels: 2,
        output_channels: 2,
        sample_rate: 48_000,
        buffer_size: BufferSize::Default,
    };

    println!("[D] Building stream ...");
    let stream = match device.build_duplex_stream_raw(
        config,
        SampleFormat::F32,
        SampleFormat::F32,
        move |input, output, _info| {
            // Can't meaningfully report from the audio thread; skip the buffer instead.
            let (Some(i), Some(o)) = (input.as_slice::<f32>(), output.as_slice_mut::<f32>())
            else {
                return;
            };

            for (d, s) in o.iter_mut().zip(i) {
                *d = *s;
            }
        },
        move |err| {
            eprintln!("[D] Stream Error: {err}");
        },
        None,
    ) {
        Ok(stream) => stream,
        Err(e) => {
            report("[D] Failed to build stream", e);
            return;
        }
    };

    println!("[D] Starting stream ...");
    if let Err(e) = stream.start() {
        report("[D] Failed to start stream", e);
        return;
    }

    println!("[D] Sleeping for 10s");
    sleep(Duration::from_secs(10));

    println!("[D] Stopping stream ...");
    if let Err(e) = stream.stop(None) {
        report("[D] Failed to stop stream", e);
    }
    println!("[D] End Duplex Feedback");
}