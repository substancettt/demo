use tokio_codec::{Decoder, Encoder};
use std::{fmt, io};
use bytes::{BufMut, BytesMut};
use bincode::config;

pub struct P2p;

pub static HEADER_LENGTH: usize = 8;
pub static NODE_ID_LENGTH: usize = 36;
pub static NODE_ID_SEC1_LENGTH: usize = 24;
pub static IP_LENGTH: usize = 8;

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Version {
    V0 = 0,
    V1 = 1,
    UNKNOWN = 0xFFFF,
}

impl Version {
    pub fn value(&self) -> u16 {
        match *self {
            Version::V0 => 0 as u16,
            Version::V1 => 1 as u16,
            Version::UNKNOWN => 0xFFFF as u16,
        }
    }
    
    pub fn get(value: u16) -> Version {
        match value {
            0 => Version::V0,
            1 => Version::V1,
            _ => Version::UNKNOWN,
        }
    }
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

impl Control {
    pub fn value(&self) -> u8 {
        match *self {
            Control::NET => 0 as u8,
            Control::SYNC => 1 as u8,
            Control::UNKNOWN => 0xFF as u8,
        }
    }
    
    pub fn get(value: u8) -> Control {
        match value {
            0 => Control::NET,
            1 => Control::SYNC,
            _ => Control::UNKNOWN,
        }
    }
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

impl Action {
    pub fn value(&self) -> u8 {
        match *self {
            Action::DISCONNECT => 0 as u8,
            Action::HANDSHAKEREQ => 1 as u8,
            Action::HANDSHAKERES => 2 as u8,
            Action::PING => 3 as u8,
            Action::PONG => 4 as u8,
            Action::ACTIVENODESREQ => 5 as u8,
            Action::ACTIVENODESRES => 6 as u8,
            Action::UNKNOWN => 0xFF as u8,
        }
    }
    
    pub fn get(value: u8) -> Action {
        match value {
            0 => Action::DISCONNECT,
            1 => Action::HANDSHAKEREQ,
            2 => Action::HANDSHAKERES,
            3 => Action::PING,
            4 => Action::PONG,
            5 => Action::ACTIVENODESREQ,
            6 => Action::ACTIVENODESRES,
            _ => Action::UNKNOWN,
        }
    }
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
pub enum SyncAction {
    STATUSREQ = 0,
    STATUSRES = 1,
    BLOCKSHEADERSREQ = 2,
    BLOCKSHEADERSRES = 3,
    BLOCKSBODIESREQ = 4,
    BLOCKSBODIESRES = 5,
    BROADCASTTX = 6,
    BROADCASTBLOCK = 7,
    UNKNOWN = 0xFF,
}


impl SyncAction {
    pub fn value(&self) -> u8 {
        match *self {
            SyncAction::STATUSREQ => 0 as u8,
            SyncAction::STATUSRES => 1 as u8,
            SyncAction::BLOCKSHEADERSREQ => 2 as u8,
            SyncAction::BLOCKSHEADERSRES => 3 as u8,
            SyncAction::BLOCKSBODIESREQ => 4 as u8,
            SyncAction::BLOCKSBODIESRES => 5 as u8,
            SyncAction::BROADCASTTX => 6 as u8,
            SyncAction::BROADCASTBLOCK => 7 as u8,
            SyncAction::UNKNOWN => 0xFF as u8,
        }
    }
    
    pub fn get(value: u8) -> SyncAction {
        match value {
            0 => SyncAction::STATUSREQ,
            1 => SyncAction::STATUSRES,
            2 => SyncAction::BLOCKSHEADERSREQ,
            3 => SyncAction::BLOCKSHEADERSRES,
            4 => SyncAction::BLOCKSBODIESREQ,
            5 => SyncAction::BLOCKSBODIESRES,
            6 => SyncAction::BROADCASTTX,
            7 => SyncAction::BROADCASTBLOCK,
            _ => SyncAction::UNKNOWN,
        }
    }
}

impl fmt::Display for SyncAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            SyncAction::STATUSREQ => "STATUSREQ",
            SyncAction::STATUSRES => "STATUSRES",
            SyncAction::BLOCKSHEADERSREQ => "BLOCKSHEADERSREQ",
            SyncAction::BLOCKSHEADERSRES => "BLOCKSHEADERSRES",
            SyncAction::BLOCKSBODIESREQ => "BLOCKSBODIESREQ",
            SyncAction::BLOCKSBODIESRES => "BLOCKSBODIESRES",
            SyncAction::BROADCASTTX => "BROADCASTTX",
            SyncAction::BROADCASTBLOCK => "BROADCASTBLOCK",
            SyncAction::UNKNOWN => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct Head {
    pub ver: u16,
    pub ctrl: u8,
    pub action: u8,
    pub len: u32,
}

impl Head {
    pub fn new() -> Head {
        Head {
            ver: Version::UNKNOWN.value(),
            ctrl: Control::UNKNOWN.value(),
            action: Action::UNKNOWN.value(),
            len: 0,
        }
    }
    
    pub fn len() -> usize {
        8 as usize
    }
}

impl fmt::Display for Head {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Version: {}, Control {}, Action {}, Length {})", self.ver, self.ctrl, self.action, self.len)
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
            try!(write!(f, "{:02X}", v));
        }
        write!(f, "\n")
    }
}

impl fmt::Display for ChannelBuffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Head: {}, Body {})", self.head, Array(self.body.to_vec()))
    }
}

#[derive(Clone, Copy, Deserialize, Hash, PartialEq, Serialize)]
pub struct IpAddr {
    pub ip: [u8; 8],
    pub port: u32,
}

impl IpAddr {
    pub fn new() -> IpAddr {
        IpAddr {
            ip: [0; 8],
            port: 0,
        }
    }
    
    pub fn get_addr(&self) -> String {
        format!("{}.{}.{}.{}:{}", self.ip[1], self.ip[3], self.ip[5], self.ip[7], self.port).to_string()
    }
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Ip Address: {}\n", self.get_addr())
    }
}

pub struct HandshakeReqBody {
    pub node_id: [u8; 36],
    pub net_id: u32,
    pub ip_addr: IpAddr,
    pub revision_version: Vec<u8>,
}

impl HandshakeReqBody {
    pub fn new() -> HandshakeReqBody {
        HandshakeReqBody {
            node_id: [b'*'; 36],
            net_id: 0,
            ip_addr: IpAddr::new(),
            revision_version: Vec::new(),
        }
    }
}

impl fmt::Display for HandshakeReqBody {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "HandshakeReqBody: \n    Node Id: "));
        for c in self.node_id.iter() {
            try!(write!(f, "{}", *c as char));
        }
        try!(write!(f, "\n"));
        try!(write!(f, "    net_id: {}\n", self.net_id));
        try!(write!(f, "    {}\n", self.ip_addr));
        try!(write!(f, "    revision & version: "));
        for sec in self.revision_version.iter() {
            try!(write!(f, "{:02X}", sec));
        }
        write!(f, "\n")
    }
}

pub struct ActiveNodesSection {
    pub node_id: [u8; 36],
    pub ip_addr: IpAddr,
}

impl ActiveNodesSection {
    pub fn new() -> ActiveNodesSection {
        ActiveNodesSection {
            node_id: [b'0'; 36],
            ip_addr: IpAddr::new(),
        }
    }
}

impl fmt::Display for ActiveNodesSection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "ActiveNodesSection: \n    Node Id: "));
        for c in self.node_id.iter() {
            try!(write!(f, "{}", *c as char));
        }
        write!(f, "    {}\n", self.ip_addr)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Node {
    pub id_sec1: [u8; 24],
    pub id_sec2: [u8; 12],
    pub ip_addr: IpAddr,
    pub node_hash: u64,
}

impl Node {
    pub fn new() -> Node {
        Node {
            id_sec1: [b'0'; 24],
            id_sec2: [b'0'; 12],
            ip_addr: IpAddr::new(),
            node_hash: 0,
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Node information: \n    Node id: "));
        for c in self.id_sec1.iter() {
            try!(write!(f, "{}", *c as char));
        }
        for c in self.id_sec2.iter() {
            try!(write!(f, "{}", *c as char));
        }
        try!(write!(f, "\n"));
        try!(write!(f, "    {}\n", self.ip_addr));
        write!(f, "    node hash: {:064X}", self.node_hash)
    }
}

impl Encoder for P2p {
    type Item = ChannelBuffer;
    type Error = io::Error;
    
    fn encode(&mut self, item: ChannelBuffer, dst: &mut BytesMut) -> io::Result<()> {
        let mut encoder = config();
        let encoder = encoder.big_endian();
        let encoded: Vec<u8> = encoder.serialize(&item.head).unwrap();
        dst.put_slice(encoded.as_slice());
        dst.put_slice(item.body.as_slice());

//        let mut i = 0;
//        for c in item.body.iter() {
//            debug!("encoded body[{}]: {:02X}", i, c);
//            i = i + 1;
//        }
        
        return Ok(());
    }
}

impl Decoder for P2p {
    type Item = ChannelBuffer;
    type Error = io::Error;
    
    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<ChannelBuffer>> {
        let len = src.len();
        if len > 0 && len != 1440 {
            
            let buf = src.split_to(len);
            debug!("Frame length: {}", buf.len());
            
            let mut decoder = config();
            let decoder = decoder.big_endian();
            
            let mut decoded = ChannelBuffer::new();
            let (head, body) = buf.split_at(HEADER_LENGTH);
            decoded.head = decoder.deserialize(head).unwrap();
            decoded.body.put_slice(body.to_vec().as_slice());

            if decoded.head.len != decoded.body.len() as u32 {
                debug!("Body length in header: {}", decoded.head.len);
                debug!("Body length: {}", decoded.body.len());
                debug!("ChannelBuffer: {}", decoded);
            }
            
            debug!("Head: {}", decoded.head);
            
            Ok(Some(decoded))
        } else {
            debug!("Empty frame ...");
            Ok(None)
        }
    }
}
