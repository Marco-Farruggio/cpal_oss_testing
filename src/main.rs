use std::{
    fmt::Display,
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering},
    },
    thread::sleep,
    time::Duration,
};

use cpal::{
    SupportedStreamConfigRange,
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
    eprintln!("[I] [!] {context}: {err}");
}

fn fmt_sup_config_rng(config: SupportedStreamConfigRange) -> String {
    let buf = match config.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => format!("{min} - {max}"),
        cpal::SupportedBufferSize::Unknown => "unk".into(),
    };

    format!("{}ch {} - {}hz {} buf: {buf}", config.channels(), config.min_sample_rate(), config.max_sample_rate(), config.sample_format())
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
        test_output(&device);
        test_input(&device);
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
                println!(" - {}", fmt_sup_config_rng(config));
            }
        }
        Err(e) => report("Failed to query supported input configs", e),
    }

    println!(" - Out:");
    match device.supported_output_configs() {
        Ok(configs) => {
            for config in configs {
                println!(" - {}", fmt_sup_config_rng(config));
            }
        }
        Err(e) => report("Failed to query supported output configs", e),
    }
}

fn test_input(device: &Device) {
    println!("[I] Starting Input Capture Test ...");
    let config = StreamConfig {
        channels: 1,
        sample_rate: 48_000,
        buffer_size: BufferSize::Default,
    };

    let ncallbacks = Arc::new(AtomicU16::new(0));
    let nsamples = Arc::new(AtomicU64::new(0));
    // Peak |sample| since last poll, stored as f32 bits. For non-negative floats,
    // the u32 bit pattern is monotonic, so fetch_max works as a float max.
    let peak_bits = Arc::new(AtomicU32::new(0));
    let nbad = Arc::new(AtomicU32::new(0)); // buffers that weren't F32

    let (cb_calls, cb_samples, cb_peak, cb_bad) = (
        ncallbacks.clone(),
        nsamples.clone(),
        peak_bits.clone(),
        nbad.clone(),
    );

    println!("[I] Building stream ...");
    let stream = match device.build_input_stream_raw(
        config,
        SampleFormat::U8,
        move |data, _info| {
            cb_calls.fetch_add(1, Ordering::Relaxed);

            // Can't meaningfully report from the audio thread; count and skip instead.
            let Some(samples) = data.as_slice::<f32>() else {
                cb_bad.fetch_add(1, Ordering::Relaxed);
                return;
            };

            cb_samples.fetch_add(samples.len() as u64, Ordering::Relaxed);

            let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
            cb_peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
        },
        |err| {
            eprintln!("[I] Stream Error: {err}");
        },
        None,
    ) {
        Ok(stream) => stream,
        Err(e) => {
            report("[I] Failed to build stream", &e);
            println!("{:?} | {:?}", e.kind(), e.message());
            return;
        }
    };

    println!("[I] Starting stream ...");
    if let Err(e) = stream.start() {
        report("[I] Failed to start stream", e);
        return;
    }

    // Poll a peak meter ~4x/sec for 3s.
    for _ in 0..12 {
        sleep(Duration::from_millis(250));
        let peak = f32::from_bits(peak_bits.swap(0, Ordering::Relaxed));
        let bars = (peak.min(1.0) * 40.0) as usize;
        println!("[I] {:<40} {peak:.4}", "#".repeat(bars));
    }

    println!("[I] Stopping stream ...");
    if let Err(e) = stream.stop(None) {
        report("[I] Failed to stop stream", e);
    }

    let calls = ncallbacks.load(Ordering::Relaxed);
    let samples = nsamples.load(Ordering::Relaxed);
    let bad = nbad.load(Ordering::Relaxed);

    println!("[I] Number of Callbacks: {calls}");
    println!("[I] Samples captured (~288000 expected for 3s 48kHz 2ch): {samples}");
    if bad > 0 {
        report("[I] Non-F32 buffers received", bad);
    }
    if calls == 0 {
        report("[I] Result", "no callbacks fired (device produced no data)");
    } else if samples == 0 {
        report("[I] Result", "callbacks fired but no samples were F32");
    }
}

fn test_output(device: &Device) {
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