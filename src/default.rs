use libc::{sa_family_t, connect, recv, sockaddr_un, socket, AF_UNIX, SOCK_SEQPACKET};
use std::ffi::CString;
use std::mem;
use std::os::unix::io::RawFd;
use std::ptr;

const DEVD_PIPE: &str = "/var/run/devd.seqpacket.pipe";

pub fn monitor() -> std::io::Result<()> {
    unsafe {
        // Create a SOCK_SEQPACKET socket
        let sockfd: RawFd = socket(AF_UNIX, SOCK_SEQPACKET, 0);
        if sockfd < 0 {
            return Err(std::io::Error::last_os_error());
        }

        // Prepare the socket address
        let mut addr: sockaddr_un = mem::zeroed();
        addr.sun_family = AF_UNIX as _;
        let path_cstr = CString::new(DEVD_PIPE).unwrap();
        ptr::copy_nonoverlapping(
            path_cstr.as_ptr() as *const u8,
            addr.sun_path.as_mut_ptr() as *mut u8,
            DEVD_PIPE.len(),
        );

        // Connect to the devd pipe
        let addr_len = (mem::size_of::<sa_family_t>() + DEVD_PIPE.len()) as u32;
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
            // Filter for sound system connection events
            if msg.contains("system=\"SND\"") && msg.contains("subsystem=\"CONN\"") {
                println!("Default audio device changed: {msg}");
            }
        }
    }
    Ok(())
}