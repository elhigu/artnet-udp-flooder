use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

use artnet_protocol::*;
use std::net::UdpSocket;

use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
struct AddressConfig {
    address: String,
    port: u16,
}

#[derive(Serialize, Deserialize, Debug)]
struct UniverseMappingConfig {
    input: (u16, u16),
    output_start: u16,
}

#[derive(Serialize, Deserialize, Debug)]
struct DeviceMappingConfig {
    host: AddressConfig,
    universes: UniverseMappingConfig,
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    listen: AddressConfig,
    mappings: Vec<DeviceMappingConfig>,
}

fn read_config_file(file_path: &str) -> std::result::Result<Config, std::io::Error> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config: Config = serde_json::from_str(&contents)?;
    println!("{:?}", config);
    Ok(config)
}

struct OutputDevice {
    address: String,

    // virtual screen where proxy writes the universes for passing them as a single frame to ESP
    // or depending on protocol may as well send them as multiple universes with fixed packet
    // headers and sync messages etc.
    frame: Vec<u8>,

    // Current sequence
    sequence: u8,

    // Number of universes configured to send to this device
    universe_count: u16,

    // thread communication and the join_handle of spawned thread, filled after thread is started
    thread_tx: Option<mpsc::Sender<Output>>,
    join_handle: Option<JoinHandle<()>>, // TODO: stats about how often actually full universe range was received
}

impl OutputDevice {
    fn new(config: &DeviceMappingConfig) -> OutputDevice {
        let universe_count = config.universes.input.1 - config.universes.input.0 + 1;
        let frame = vec![0; (universe_count as usize) * 510];

        OutputDevice {
            address: format!("{}:{}", &config.host.address, &config.host.port),
            frame,
            sequence: 0,
            universe_count,
            thread_tx: Option::None,
            join_handle: Option::None,
        }
    }

    fn next_sequence(&mut self) -> u8 {
        if self.sequence == 255 {
            self.sequence = 1;
        } else {
            self.sequence += 1;
        }
        return self.sequence;
    }

    fn send_frame(&mut self) {
        // TODO: take mutex to lock thread accessing self.frame and self.send_queue
        for universe in 0..self.universe_count {
            let start: usize = universe as usize * 510;
            let end = start + 510;
            let data: Vec<u8> = self.frame[start..end].to_vec();

            let mut output = Output {
                data: data.into(),
                ..Output::default()
            };

            // TODO: add output offset
            output.port_address = PortAddress::try_from(universe).unwrap();
            output.sequence = self.next_sequence();

            self.thread_tx.as_mut().unwrap().send(output).unwrap();
        }
    }

    fn start_output_thread(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.thread_tx = Some(tx);
        let address = self.address.to_owned();

        let join_handle = thread::spawn(move || {
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            loop {
                for output in &rx {
                    // TODO: if output is Option::None break loop
                    let bytes = ArtCommand::Output(output).write_to_buffer().unwrap();
                    socket.send_to(&bytes, &address).unwrap();
                    // 2000 packet / s should be inaf
                    thread::sleep(Duration::from_micros(500));
                }
                // TODO: add sync message?
            }
        });

        self.join_handle = Some(join_handle);
    }
}

struct Outputs {
    devices: Vec<OutputDevice>,
}

impl Outputs {
    fn new(config: &Vec<DeviceMappingConfig>) -> Outputs {
        let mut devices: Vec<OutputDevice> = Vec::new();

        let mut device_by_port = HashMap::new();

        for device_config in config {
            // add mapping to setup ports which universes should be delivered to this device
            let input_range = device_config.universes.input.0..=device_config.universes.input.1;
            for port in input_range {
                // TODO: learn how to deal with multiple references to a same data and how to
                //       bind lifespan properly
                device_by_port.insert(port, devices.len());
            }

            let mut device = OutputDevice::new(&device_config);
            device.start_output_thread();
            devices.push(device);
        }
        Outputs { devices }
    }

    fn trigger_frames(&mut self) {
        for device in &mut self.devices {
            device.send_frame();
        }
    }
}

fn main() {
    let config = read_config_file("config.json").unwrap();
    let mut outputs = Outputs::new(&config.mappings);

    loop {
        // 100fps N universes depending on config (currently 16 universes)
        outputs.trigger_frames();
        thread::sleep(Duration::from_millis(25));
    }
}
