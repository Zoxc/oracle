fn checksum(buffer: &mut [u8], sum: &mut u32) {
    for word in buffer.chunks(2) {
        let mut part = u16::from(word[0]) << 8;
        if word.len() > 1 {
            part += u16::from(word[1]);
        }
        *sum = sum.wrapping_add(u32::from(part));
    }
}

fn finish_checksum(buffer: &mut [u8], sum: &mut u32) -> u16 {
    while (*sum >> 16) > 0 {
        *sum = (*sum & 0xffff) + (*sum >> 16);
    }

    !*sum as u16
}

fn write_checksum(buffer: &mut [u8], sum: &mut u32) {
    checksum(buffer, sum);
    let sum = finish_checksum(buffer, sum);

    buffer[2] = (sum >> 8) as u8;
    buffer[3] = (sum & 0xff) as u8;
}

pub fn ping() {
    use byteorder::{BigEndian, WriteBytesExt};
    use socket2::{Domain, Protocol, Socket, Type};
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::net::{SocketAddrV4, SocketAddrV6};

    let socket = Socket::new(Domain::ipv4(), Type::raw(), Some(Protocol::icmpv4())).unwrap();

    let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0);

    // On Windows replies include IPv4 header, keeps seq and identifier. Other apps pings are not picked up. Checksum not calculated in OS.

    // Linux, replies include IPv4 header, keeps seq and identifier. See other apps' pings. Sees our own request too. Requires root. Checksum not calculated in OS.

    const ECHO_REQUEST_TYPE: u8 = 8;
    const ECHO_REQUEST_CODE: u8 = 0;

    let mut buf: Vec<u8> = Vec::new();

    buf.write_u8(ECHO_REQUEST_TYPE).unwrap();
    buf.write_u8(ECHO_REQUEST_CODE).unwrap();
    buf.write_u16::<BigEndian>(0).unwrap();
    buf.write_u16::<BigEndian>(1).unwrap();
    buf.write_u16::<BigEndian>(2).unwrap();

    write_checksum(&mut buf, &mut 0);

    println!("sending ipv4 {:x?} to {:?} ", buf, addr);

    socket.send_to(&buf, &addr.into()).unwrap();

    // IPV6

    // On Windows replies exclude IPv6 header, keeps seq and identifier. Other apps pings are not picked up. Checksum calculated in OS.

    // Linux, replies exclude IPv6 header, keeps seq and identifier. See other apps' pings. Sees our own request too. Checksum calculated in OS. Requires root.

    let socket6 = Socket::new(Domain::ipv6(), Type::raw(), Some(Protocol::icmpv6())).unwrap();

    let addr = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 0, 0, 0);

    let mut buf: Vec<u8> = Vec::new();

    buf.write_u8(128).unwrap();
    buf.write_u8(0).unwrap();
    buf.write_u16::<BigEndian>(0).unwrap();
    buf.write_u16::<BigEndian>(1).unwrap();
    buf.write_u16::<BigEndian>(2).unwrap();

    //write_checksum(&mut buf, &mut 0);

    println!("sending ipv6 {:x?} to {:?} ", buf, addr);

    socket6.send_to(&buf, &addr.into()).unwrap();

    std::thread::spawn(move || loop {
        let mut recv = [0; 1500];

        if let Ok((size, src)) = socket6.recv_from(&mut recv) {
            let data = &recv[0..size];
            println!("saw ipv6 {:x?} from {:?} ", data, src);
        }
    });

    // Packets
    /*
        let socket6 = Socket::new(Domain::ipv6(), Type::raw(), Some(Protocol::raw())).unwrap();

        let addr = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 0, 0, 0);

        let mut buf: Vec<u8> = Vec::new();

        buf.write_u8(128).unwrap();
        buf.write_u8(0).unwrap();
        buf.write_u16::<BigEndian>(0).unwrap();
        buf.write_u16::<BigEndian>(1).unwrap();
        buf.write_u16::<BigEndian>(2).unwrap();

        //write_checksum(&mut buf, &mut 0);

        println!("sending ipv6 {:x?} to {:?} ", buf, addr);

        socket6.send_to(&buf, &addr.into()).unwrap();

        std::thread::spawn(move || loop {
            let mut recv = [0; 1500];

            if let Ok((size, src)) = socket6.recv_from(&mut recv) {
                let data = &recv[0..size];
                println!("saw ipv6 {:x?} from {:?} ", data, src);
            }
        });
    */
    loop {
        let mut recv = [0; 1500];

        if let Ok((size, src)) = socket.recv_from(&mut recv) {
            let data = &recv[0..size];
            println!("saw ipv4 {:x?} from {:?} ", data, src);
        }
    }
}
