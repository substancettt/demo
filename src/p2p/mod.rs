use tokio_codec::{Decoder, Encoder};
use std::{fmt, io};
use bytes::{BufMut, BytesMut};
use bincode::{serialize, deserialize};

pub struct P2p;

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Version {
    V0 = 0,
    V1 = 1,
    UNKNOWN = 0xFFFF,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Version::V0 => "V0",
            Version::V1 => "V1",
            Version::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Control {
    NET = 0,
    SYNC = 1,
    UNKNOWN = 0xFF,
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Control::NET => "NET",
            Control::SYNC => "SYNC",
            Control::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Action {
    DISCONNECT = 0,
    HANDSHAKEREQ = 1,
    HANDSHAKERES = 2,
    PING = 3,
    PONG = 4,
    ACTIVENODESREQ = 5,
    ACTIVENODESRES = 6,
    UNKNOWN = 0xFF,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            Action::DISCONNECT => "DISCONNECT",
            Action::HANDSHAKEREQ => "HANDSHAKEREQ",
            Action::HANDSHAKERES => "HANDSHAKERES",
            Action::PING => "PING",
            Action::PONG => "PONG",
            Action::ACTIVENODESREQ => "ACTIVENODESREQ",
            Action::ACTIVENODESRES => "ACTIVENODESRES",
            Action::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct Head {
    pub ver: Version,
    pub ctrl: Control,
    pub action: Action,
    pub node_id_hash: u64,
    pub len: u32,
}

impl Head {
    pub fn new() -> Head {
        Head {
            ver: Version::UNKNOWN,
            ctrl: Control::UNKNOWN,
            action: Action::UNKNOWN,
            node_id_hash: 0,
            len: 0,
        }
    }
}

impl fmt::Display for Head {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Version: {}, Control {}, Action {}, NodeIdHash {:064X}, Lenght {})", self.ver, self.ctrl, self.action, self.node_id_hash, self.len)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct ChannelBuffer {
    pub head: Head,
    pub body: Vec<u8>,
}

impl ChannelBuffer {
    pub fn new() -> ChannelBuffer {
        ChannelBuffer {
            head: Head::new(),
            body: Vec::new(),
        }
    }
}

struct Array(Vec<u8>);

impl fmt::Display for Array {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Array(ref vec) = *self;
        for (count, v) in vec.iter().enumerate() {
            if count != 0 { try!(write!(f, " ")); }
            try!(write!(f, "{}", v));
        }
        write!(f, "\n")
    }
}

impl fmt::Display for ChannelBuffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Head: {}, Body {})", self.head, Array(self.body.to_vec()))
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct HandshakeReqBody {
    pub node_id_sec1: [u8; 8],
    node_id_dem1: u8,
    pub node_id_sec2: [u8; 4],
    node_id_dem2: u8,
    pub node_id_sec3: [u8; 4],
    node_id_dem3: u8,
    pub node_id_sec4: [u8; 4],
    node_id_dem4: u8,
    pub node_id_sec5: [u8; 12],
    pub net_id: u32,
    pub ip: [u8; 8],
    pub port: u32,
    pub revision_version: Vec<u8>,
}

impl HandshakeReqBody {
    pub fn new() -> HandshakeReqBody {
        HandshakeReqBody {
            node_id_sec1: [0; 8],
            node_id_dem1: b'-',
            node_id_sec2: [0; 4],
            node_id_dem2: b'-',
            node_id_sec3: [0; 4],
            node_id_dem3: b'-',
            node_id_sec4: [0; 4],
            node_id_dem4: b'-',
            node_id_sec5: [0; 12],
            net_id: 0,
            ip: [0; 8],
            port: 0,
            revision_version: Vec::new(),
        }
    }
}

impl fmt::Display for HandshakeReqBody {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "HandshakeReqBody: \n    Node id: "));
        for sec in self.node_id_sec1.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.node_id_sec2.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.node_id_sec3.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.node_id_sec4.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.node_id_sec5.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "\n"));

        try!(write!(f, "    net_id: {}\n", self.net_id));

        try!(write!(f, "    ip: "));
        for sec in self.ip.iter() {
            try!(write!(f, "{:02X}", sec));
        }
        try!(write!(f, "\n"));

        try!(write!(f, "    port: {}\n", self.port));

        try!(write!(f, "    revision & version: "));
        for sec in self.revision_version.iter() {
            try!(write!(f, "{:02X}", sec));
        }
        write!(f, "\n")
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct HandshakeResBody {
    pub status: u8,
    pub len: u8,
    pub binary_version: Vec<u8>,
}

impl HandshakeResBody {
    pub fn new() -> HandshakeResBody {
        HandshakeResBody {
            status: 0,
            len: 0,
            binary_version: Vec::new(),
        }
    }
}

impl fmt::Display for HandshakeResBody {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "HandshakeResBody: \n    Status: "));
        try!(write!(f, "{}\n", self.status));
        try!(write!(f, "    Length: {}\n", self.len));
        
        try!(write!(f, "    binary_version: "));
        for sec in self.binary_version.iter() {
            try!(write!(f, "{:02X}", sec));
        }
        write!(f, "\n")
    }
}

#[derive(Clone, Copy, Deserialize, Hash, PartialEq, Serialize)]
pub struct Node {
    pub id_sec1: [u8; 8],
    pub id_sec2: [u8; 4],
    pub id_sec3: [u8; 4],
    pub id_sec4: [u8; 4],
    pub id_sec5: [u8; 12],
    pub ip: [u8; 8],
    pub port: u32,
    pub id_hash: u64,
}

impl Node {
    pub fn new() -> Node {
        Node {
            id_sec1: [b'*'; 8],
            id_sec2: [b'*'; 4],
            id_sec3: [b'*'; 4],
            id_sec4: [b'*'; 4],
            id_sec5: [b'*'; 12],
            ip: [0; 8],
            port: 0,
            id_hash: 0,
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Node information: \n    Node id: "));
        for sec in self.id_sec1.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.id_sec2.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.id_sec3.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.id_sec4.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "-"));
        for sec in self.id_sec5.iter() {
            try!(write!(f, "{}", *sec as char));
        }
        try!(write!(f, "\n"));
        
        try!(write!(f, "    ip: "));
        for sec in self.ip.iter() {
            try!(write!(f, "{:02X}", sec));
        }
        try!(write!(f, "\n"));
        
        try!(write!(f, "    port: {}\n", self.port));
        try!(write!(f, "    id_hash: {:064X}\n", self.id_hash));

        write!(f, "\n")
    }
}

#[derive(Clone, Copy)]
pub struct PeerInfo {
    pub ip: &'static  str,
    pub port: u32,
    pub node_id: &'static  str,
}

impl PeerInfo {
    pub fn new(_ip: &'static str, _port: u32, _node_id: &'static str) -> PeerInfo {
        PeerInfo {
            ip: _ip.into(),
            port: _port,
            node_id: _node_id.into(),
        }
    }
}

impl Encoder for P2p {
    type Item = ChannelBuffer;
    type Error = io::Error;

    fn encode(&mut self, item: ChannelBuffer, dst: &mut BytesMut) -> io::Result<()> {
        let encoded: Vec<u8> = serialize(&item).unwrap();
        dst.put_slice(encoded.as_slice());

        return Ok(());
    }
}

impl Decoder for P2p {
    type Item = ChannelBuffer;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<ChannelBuffer>> {
        let len = src.len();
        if src.len() > 0 {
            let encoded: Vec<u8> = src.split_to(len).to_vec();
            let decoded: ChannelBuffer = deserialize(&encoded[..]).unwrap();

            Ok(Some(decoded))
        } else {
            Ok(None)
        }
    }
}
