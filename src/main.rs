use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

use artnet_protocol::*;
use std::net::UdpSocket;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

#[derive(Serialize, Deserialize, Debug)]
struct AddressConfig {
    address: String,
    port: u16,
}

#[derive(Serialize, Deserialize, Debug)]
struct DeviceMappingConfig {
    host: AddressConfig,
    throttle_us: u64,
    universe_count: u16,
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    listen: AddressConfig,
    fps: f32,
    outputs: Vec<DeviceMappingConfig>,
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

    //Virtual screen where proxy writes the universes for passing them as a single frame to ESP
    // or depending on protocol may as well send them as multiple universes with fixed packet
    // headers and sync messages etc. I could also generate some test visuals here, but maybe laterz
    frame: Vec<u8>,

    // Current sequence number for outgoing artnet packet
    sequence: u8,

    // Number of universes configured to send to this device
    universe_count: u16,

    // Number of microseconds to wait after sending a packet to the output host
    throttle_us: u64,

    // thread communication and the join_handle of spawned thread, filled after thread is started
    thread_tx: Option<mpsc::Sender<Output>>,
    join_handle: Option<JoinHandle<()>>,

    // for stats counting output packets per second
    sent_universes: Arc<Mutex<u32>>,
}

impl OutputDevice {
    fn new(config: &DeviceMappingConfig) -> OutputDevice {
        let universe_count = config.universe_count;
        let frame = vec![0; (universe_count as usize) * 510];

        OutputDevice {
            address: format!("{}:{}", &config.host.address, &config.host.port),
            frame,
            sequence: 0,
            throttle_us: config.throttle_us,
            universe_count,
            thread_tx: Option::None,
            join_handle: Option::None,
            sent_universes: Arc::new(Mutex::new(0)),
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
        for universe in 0..self.universe_count {
            let start: usize = universe as usize * 510;
            let end = start + 510;
            let data: Vec<u8> = self.frame[start..end].to_vec();

            let mut output = Output {
                data: data.into(),
                ..Output::default()
            };

            output.port_address = PortAddress::try_from(universe).unwrap();
            output.sequence = self.next_sequence();

            self.thread_tx.as_mut().unwrap().send(output).unwrap();
        }
    }

    fn dump_report(&mut self, elapsed_milliseconds: u128) {
        let sent_universes = self.sent_universes.clone();
        let sent_universes_count: u32;

        // get number of sent universes and reset counter
        {
            let mut locked_count = sent_universes.lock().unwrap();
            sent_universes_count = *locked_count;
            *locked_count = 0;
        }

        let universes_per_second =
            (sent_universes_count as f64) / ((elapsed_milliseconds as f64) / 1000f64);

        println!(
            "Sending to {:24} packets:{:8.2}/sec payload:{:8.2} Mbps",
            self.address,
            universes_per_second,
            universes_per_second * 530f64 * 8f64 / 1024f64 / 1024f64
        );
    }

    fn start_output_thread(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.thread_tx = Some(tx);
        let address = self.address.to_owned();

        let sent_universes = self.sent_universes.clone();
        let throttle_us: u64 = self.throttle_us;

        let join_handle = thread::spawn(move || {
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            loop {
                for output in &rx {
                    // if sleeping after every packet, context switch delays seems to
                    // limit output to ~1800 packets / sec so we allow sending 5 packets
                    // per switch.
                    let should_sleep_after = (output.sequence % 5) == 0;
                    let bytes = ArtCommand::Output(output).write_to_buffer().unwrap();
                    socket.send_to(&bytes, &address).unwrap();

                    {
                        let mut locked_count = sent_universes.lock().unwrap();
                        *locked_count += 1;
                    }

                    if should_sleep_after {
                        thread::sleep(Duration::from_micros(throttle_us));
                    }
                }

                // TODO: add output sync message after frame is complete
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

        for device_config in config {
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

    fn dump_reports(&mut self, elapsed_milliseconds: u128) {
        for device in &mut self.devices {
            device.dump_report(elapsed_milliseconds);
        }
    }
}

fn main() {
    let config = read_config_file("config.json").unwrap();
    let mut outputs = Outputs::new(&config.outputs);
    let mut last_report = SystemTime::now();

    let wait_milliseconds = (1f32 / config.fps) * 1000f32;

    loop {
        outputs.trigger_frames();

        // not very accurate, but close inaf I suppose... maybe should actually measure how often frames are really triggered
        thread::sleep(Duration::from_millis(wait_milliseconds.round() as u64));

        // debug print of outgoing packets rate
        let since_last_report_ms = last_report.elapsed().unwrap().as_millis();
        if since_last_report_ms > 1000 {
            last_report = SystemTime::now();
            outputs.dump_reports(since_last_report_ms);
        }
    }
}
