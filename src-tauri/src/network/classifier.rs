use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};


#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TrafficType {
    TcpTor,
    UdpAmneziaWG,
    Dns,
    Local,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedPacket {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub traffic_type: TrafficType,
}

pub struct TrafficClassifier {
    tor_socks_port: u16,
    dns_port: u16,
    local_nets: Vec<(IpAddr, u8)>,
    udp_ports: std::collections::HashSet<u16>,
}

impl TrafficClassifier {
    pub fn new() -> Self {
        let mut udp_ports = std::collections::HashSet::new();
        udp_ports.insert(3478);
        udp_ports.insert(3479);
        udp_ports.insert(5349);
        udp_ports.insert(5350);
        udp_ports.insert(1194);
        udp_ports.insert(1195);
        udp_ports.insert(27015..=27030);
        udp_ports.insert(4380);

        Self {
            tor_socks_port: 9050,
            dns_port: 53,
            local_nets: vec![
                (IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)), 8),
                (IpAddr::V4(Ipv4Addr::new(172, 16, 0, 0)), 12),
                (IpAddr::V4(Ipv4Addr::new(192, 168, 0, 0)), 16),
                (IpAddr::V4(Ipv4Addr::new(127, 0, 0, 0)), 8),
                (IpAddr::V6(Ipv6Addr::new(0xfd, 0, 0, 0, 0, 0, 0, 0)), 8),
                (IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0)), 10),
                (IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 128),
            ],
        }
    }

    pub fn classify(
        &self,
        src_ip: IpAddr,
        dst_ip: IpAddr,
        src_port: u16,
        dst_port: u16,
        protocol: u8,
    ) -> TrafficType {
        let packet = ClassifiedPacket {
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            protocol,
            traffic_type: TrafficType::Unknown,
        };

        self.classify_packet(&packet)
    }

    pub fn classify_packet(&self, packet: &ClassifiedPacket) -> TrafficType {
        if self.is_local_address(&packet.dst_ip) {
            return TrafficType::Local;
        }

        if packet.dst_port == self.dns_port || packet.dst_port == 853 {
            return TrafficType::Dns;
        }

        if packet.protocol == 17 {
            if self.is_voice_or_media_port(packet.dst_port) {
                return TrafficType::UdpAmneziaWG;
            }
            if self.is_voice_or_media_port(packet.src_port) {
                return TrafficType::UdpAmneziaWG;
            }
            return TrafficType::UdpAmneziaWG;
        }

        if packet.protocol == 6 {
            return TrafficType::TcpTor;
        }

        TrafficType::Unknown
    }

    fn is_local_address(&self, addr: &IpAddr) -> bool {
        self.local_nets.iter().any(|(net, prefix)| {
            match (addr, net) {
                (IpAddr::V4(ip), IpAddr::V4(net)) => {
                    let bits = *prefix as u32;
                    if bits == 0 {
                        return false;
                    }
                    let mask = if bits >= 32 {
                        u32::MAX
                    } else {
                        !((1u32 << (32 - bits)) - 1)
                    };
                    (u32::from(*ip) & mask) == (u32::from(*net) & mask)
                }
                (IpAddr::V6(ip), IpAddr::V6(net)) => {
                    if *prefix == 0 {
                        return false;
                    }
                    let ip_bytes = ip.octets();
                    let net_bytes = net.octets();
                    let full_bytes = (*prefix / 8) as usize;
                    let remaining_bits = (*prefix % 8) as u8;
                    if ip_bytes[..full_bytes] != net_bytes[..full_bytes] {
                        return false;
                    }
                    if remaining_bits > 0 && full_bytes < 16 {
                        let mask = !((1u8 << (8 - remaining_bits)) - 1);
                        (ip_bytes[full_bytes] & mask) == (net_bytes[full_bytes] & mask)
                    } else {
                        true
                    }
                }
                _ => false,
            }
        })
    }

    fn is_voice_or_media_port(&self, port: u16) -> bool {
        self.udp_ports.contains(&port)
            || (3478..=3481).contains(&port)
            || (5000..=6000).contains(&port)
            || (16384..=32767).contains(&port)
    }
}

impl Default for TrafficClassifier {
    fn default() -> Self {
        Self::new()
    }
}
