use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig, BufferSize, SampleFormat};
use libc::{connect, recv, sockaddr_un, socket, AF_UNIX, SOCK_SEQPACKET};
use std::mem;
use std::os::unix::io::RawFd;
use std::ptr;

const DEVD_PIPE: &str = "/var/run/devd.seqpacket.pipe";

pub fn monitor() -> std::io::Result<()> {
    unsafe {
        let sockfd: RawFd = socket(AF_UNIX, SOCK_SEQPACKET, 0);
        if sockfd < 0 {
            return Err(std::io::Error::last_os_error());
        }

        let mut addr: sockaddr_un = mem::zeroed();
        addr.sun_family = AF_UNIX as libc::sa_family_t;

        let path_bytes = DEVD_PIPE.as_bytes();
        assert!(path_bytes.len() < addr.sun_path.len());
        ptr::copy_nonoverlapping(
            path_bytes.as_ptr(),
            addr.sun_path.as_mut_ptr() as *mut u8,
            path_bytes.len(),
        );

        // The offset of sun_path is 2 bytes (sun_len + sun_family)
        let addr_len = (2 + path_bytes.len()) as libc::socklen_t;
        if connect(
            sockfd,
            &addr as *const sockaddr_un as *const _,
            addr_len,
        ) < 0
        {
            return Err(std::io::Error::last_os_error());
        }

        let mut buf = [0u8; 4096];
        loop {
            let n = recv(sockfd, buf.as_mut_ptr() as *mut _, buf.len(), 0);
            if n <= 0 {
                if n == 0 {
                    eprintln!("devd pipe closed");
                } else {
                    eprintln!("recv error: {}", std::io::Error::last_os_error());
                }
                break;
            }

            let msg = String::from_utf8_lossy(&buf[..n as usize]);

            // Correct filter: use unquoted `system=SND` and check for IN/OUT types.
            if msg.contains("system=SND")
                && msg.contains("subsystem=CONN")
                && (msg.contains("type=IN") || msg.contains("type=OUT"))
            {
                println!("Audio device topology changed:\n{msg}");
                // You can now parse `cdev=dspN` out of the message if needed.
            }
        }
    }
    Ok(())
}

pub fn test_default_output() {
    let host = cpal::default_host();

    // should cpal diferentiate between the current output device (static) and a dynamic handle?
    let device = host.default_output_device().unwrap();
    let config = StreamConfig {
        channels: 2,
        sample_rate: 48_000,
        buffer_size: BufferSize::Default,
    };

    let mut phase = 0f32;

    let stream = device.build_output_stream_raw(
        config,
        SampleFormat::F32,
        move |data, _info| {
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
            eprintln!("[defO] Stream Error: {err}");
        },
        None
    ).unwrap();

    stream.start().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(180)); // 3mins
}