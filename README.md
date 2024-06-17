# Art-Net UDP flooder

Server which sends art-net packets to configured hosts. App sends 5 packet bursts to the output host
and then sleeps amount of throttle_us microseconds.

System does not sleep after every packet, because otherwise windows task scheduler frequency will be limiting
factor how many packets can be sent out (around 1800/sec was maximum if sleeping after every packet).

## Getting started

    cargo build
    cargo run

## Configuration

    {
        "listen": {
            "//": "Not really used for anything",
            "address": "0.0.0.0",
            "port": 6666
        },
        "//": "How often to trigger new output frame sending",
        "fps": 40.0,
        "outputs": [
                {
                "host": {
                    "address": "192.168.137.23",
                    "port": 6454
                },
                "//": "number of microseconds to wait before sending next 5 packets to the host",
                "throttle_us": 200,
                "//": "Number of universes to send to output per frame",
                "universe_count": 8
            }
        ]
    }
