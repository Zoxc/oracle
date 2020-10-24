use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use rand;
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::io::Cursor;
use std::io::{Seek, SeekFrom};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::net::{SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct Ping {
    tx: mpsc::Sender<(Ipv4Addr, oneshot::Sender<Duration>)>,
}

impl Ping {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(1000);

        tokio::spawn(ping_task(rx));

        Self { tx }
    }

    // TODO: Remove &mut in Tokio 0.3
    pub async fn ping(&mut self, ip: Ipv4Addr) -> Duration {
        let (tx, rx) = oneshot::channel();
        self.tx.send((ip, tx)).await.unwrap();
        rx.await.unwrap()
    }
}

async fn ping_task(mut ping_requests: mpsc::Receiver<(Ipv4Addr, oneshot::Sender<Duration>)>) {
    let socket =
        Arc::new(Socket::new(Domain::ipv4(), Type::raw(), Some(Protocol::icmpv4())).unwrap());

    // Create thread to recieve ping replies
    let (mut tx, mut ping_replies) = mpsc::channel(1000);
    let socket_ = socket.clone();
    let handle = Handle::current();
    thread::spawn(move || {
        let mut buffer = [0; 1500];
        loop {
            if let Ok((size, src)) = socket_.recv_from(&mut buffer) {
                let time = SystemTime::now();
                src.as_inet().map(|sa| {
                    parse_ping_v4(&buffer[0..size]).map(|(id, seq)| {
                        handle
                            .block_on(tx.send((sa.ip().to_owned(), id, seq, time)))
                            .ok();
                    });
                });
            }
        }
    });

    // Create thread to send ping packets
    let (mut ping_sender, mut rx) = mpsc::channel(1000);
    let handle = Handle::current();
    thread::spawn(move || {
        let mut buffer = [0; 1500];
        loop {
            if let Some((ip, id, seq)) = handle.block_on(rx.recv()) {
                send_ping_v4(&socket, &mut buffer, ip, id, seq);
            }
        }
    });

    let mut seq = rand::random();
    let id = rand::random();

    let mut map: HashMap<u16, (Ipv4Addr, SystemTime, oneshot::Sender<Duration>)> = HashMap::new();

    loop {
        tokio::select! {
            Some((ip, reply)) = ping_requests.recv() => {
                map.insert(seq, (ip, SystemTime::now(), reply));
                ping_sender.send((ip, id, seq)).await.unwrap();
                seq = seq.wrapping_add(1);
            },
            Some((from, reply_id, seq, time)) = ping_replies.recv() => {
                if id == reply_id {
                    map.remove(&seq).map(|(ip, start, reply)| {
                        if ip == from {
                            let duration = time.duration_since(start).unwrap_or(Duration::from_secs(0));
                            reply.send(duration).ok();
                        }
                    });
                }
            },
            else => { break }
        };
    }
}

pub fn send_ping_v4(socket: &Socket, buffer: &mut [u8], ip: Ipv4Addr, id: u16, seq: u16) {
    const ECHO_REQUEST_TYPE: u8 = 8;
    const ECHO_REQUEST_CODE: u8 = 0;

    let addr = SocketAddrV4::new(ip, 0);

    let mut cursor = Cursor::new(buffer);

    cursor.write_u8(ECHO_REQUEST_TYPE).unwrap();
    cursor.write_u8(ECHO_REQUEST_CODE).unwrap();
    cursor.write_u16::<BigEndian>(0).unwrap();
    cursor.write_u16::<BigEndian>(id).unwrap();
    cursor.write_u16::<BigEndian>(seq).unwrap();

    let pos = cursor.position() as usize;
    let buffer = &mut cursor.into_inner()[0..pos];

    write_checksum(buffer, &mut 0);

    socket.send_to(buffer, &addr.into()).unwrap();
}

pub fn parse_ping_v4(packet: &[u8]) -> Option<(u16, u16)> {
    const ECHO_REPLY_TYPE: u8 = 0;
    const ECHO_REPLY_CODE: u8 = 0;

    let mut cursor = Cursor::new(packet);

    let ipv4_header_len = (cursor.read_u8().ok()? & 0xF) * 4;

    cursor.seek(SeekFrom::Start(ipv4_header_len as u64)).ok()?;

    if cursor.read_u8().ok()? != ECHO_REPLY_TYPE {
        return None;
    }

    if cursor.read_u8().ok()? != ECHO_REPLY_CODE {
        return None;
    }

    let _checksum = cursor.read_u16::<BigEndian>().ok()?;
    let id = cursor.read_u16::<BigEndian>().ok()?;
    let seq = cursor.read_u16::<BigEndian>().ok()?;

    if packet.len() != cursor.position() as usize {
        return None;
    }

    Some((id, seq))
}

fn checksum(buffer: &mut [u8], sum: &mut u32) {
    for word in buffer.chunks(2) {
        let mut part = u16::from(word[0]) << 8;
        if word.len() > 1 {
            part += u16::from(word[1]);
        }
        *sum = sum.wrapping_add(u32::from(part));
    }
}

fn finish_checksum(sum: &mut u32) -> u16 {
    while (*sum >> 16) > 0 {
        *sum = (*sum & 0xffff) + (*sum >> 16);
    }

    !*sum as u16
}

fn write_checksum(buffer: &mut [u8], sum: &mut u32) {
    checksum(buffer, sum);
    let sum = finish_checksum(sum);

    buffer[2] = (sum >> 8) as u8;
    buffer[3] = (sum & 0xff) as u8;
}

pub fn _ping_test() {
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
