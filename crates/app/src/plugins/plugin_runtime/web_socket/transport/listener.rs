use socket2::{Domain, Socket, Type};
use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener},
};

pub(super) fn bind(port: u16) -> io::Result<TcpListener> {
    let (socket, address) = match dual_stack_socket() {
        Ok(socket) => (socket, SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))),
        Err(_) => (
            Socket::new(Domain::IPV4, Type::STREAM, None)?,
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)),
        ),
    };
    configure_endpoint(&socket)?;
    socket.bind(&address.into())?;
    socket.listen(16)?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

fn dual_stack_socket() -> io::Result<Socket> {
    let socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;
    socket.set_only_v6(false)?;
    Ok(socket)
}

#[cfg(unix)]
fn configure_endpoint(socket: &Socket) -> io::Result<()> {
    socket.set_reuse_address(true)
}

#[cfg(windows)]
fn configure_endpoint(socket: &Socket) -> io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    use windows::Win32::Networking::WinSock::{
        SO_EXCLUSIVEADDRUSE, SOCKET, SOL_SOCKET, WSAGetLastError, setsockopt,
    };
    let handle = SOCKET(socket.as_raw_socket() as usize);
    // SAFETY: socket owns a live Winsock handle. This option accepts the
    // four-byte Win32 BOOL and Winsock reads it before returning.
    let result = unsafe {
        setsockopt(
            handle,
            SOL_SOCKET,
            SO_EXCLUSIVEADDRUSE,
            Some(&1_i32.to_ne_bytes()),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        // SAFETY: this reads the calling thread's Winsock error.
        Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError().0 }))
    }
}
