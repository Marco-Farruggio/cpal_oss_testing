use libc::{connect, recv, sockaddr_un, socket, AF_UNIX, SOCK_SEQPACKET};
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

        // Prepare the socket address (zeroed => sun_path is NUL-terminated)
        let mut addr: sockaddr_un = mem::zeroed();
        addr.sun_family = AF_UNIX as libc::sa_family_t;

        let path_bytes = DEVD_PIPE.as_bytes();
        // Leave room for the trailing NUL that mem::zeroed() already placed there
        assert!(path_bytes.len() < addr.sun_path.len());
        ptr::copy_nonoverlapping(
            path_bytes.as_ptr(),
            addr.sun_path.as_mut_ptr() as *mut u8,
            path_bytes.len(),
        );

        // Connect to the devd pipe.
        // Use full struct size, matching the C `sizeof(sun)`.
        let addr_len = mem::size_of::<sockaddr_un>() as libc::socklen_t;
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
            if msg.contains("system=\"SND\"") && msg.contains("subsystem=\"CONN\"") {
                println!("Audio device topology changed:\n{msg}");
                // parse `cdev="dspN"` out of the message, or just
                // re-read hw.snd.default_unit via sysctl
            }
        }
    }
    Ok(())
}